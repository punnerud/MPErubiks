//! getUserMedia camera glue. The <video> element stays hidden in the DOM
//! (1px, opacity 0 — display:none would stop playback in some browsers);
//! the visible preview is a zero-copy GPU texture drawn by the scan screen.
//! Pixel readback happens only for the nine small classification patches,
//! through a small willReadFrequently 2d canvas.

use eframe::wasm_bindgen::{JsCast as _, JsValue};
use wasm_bindgen_futures::JsFuture;

pub struct Camera {
    video: web_sys::HtmlVideoElement,
    canvas: web_sys::HtmlCanvasElement,
    ctx: web_sys::CanvasRenderingContext2d,
    /// Small canvas the guide square is downscaled into for grid
    /// auto-alignment (cheap 120x120 readback per tick).
    align_canvas: web_sys::HtmlCanvasElement,
    align_ctx: web_sys::CanvasRenderingContext2d,
    stream: web_sys::MediaStream,
}

/// Resolution of the alignment readback.
const ALIGN_SIZE: u32 = 120;

/// Working resolution: frames land on the canvas at this width — the SAME
/// canvas feeds both the on-screen preview and the color voting, so what
/// you see is exactly what is sampled (orientation included: 2D canvas
/// drawImage applies the platform's frame orientation, unlike raw GPU
/// copies from the <video> element, which bypass it on iOS).
const SAMPLE_WIDTH: u32 = 720;

fn js_err(e: JsValue) -> String {
    e.as_string().unwrap_or_else(|| format!("{e:?}"))
}

impl Camera {
    pub async fn open() -> Result<Camera, String> {
        let window = web_sys::window().ok_or("no window")?;
        let document = window.document().ok_or("no document")?;

        let video: web_sys::HtmlVideoElement = document
            .create_element("video")
            .map_err(js_err)?
            .dyn_into()
            .map_err(|_| "not a video element")?;
        video.set_autoplay(true);
        video.set_muted(true);
        // playsinline is essential on iOS Safari (else it force-fullscreens).
        video.set_attribute("playsinline", "true").map_err(js_err)?;
        video
            .set_attribute(
                "style",
                "position:absolute;width:1px;height:1px;opacity:0;pointer-events:none;",
            )
            .map_err(js_err)?;
        document
            .body()
            .ok_or("no body")?
            .append_child(&video)
            .map_err(js_err)?;

        let constraints = web_sys::MediaStreamConstraints::new();
        let video_opts = js_sys::Object::new();
        js_sys::Reflect::set(
            &video_opts,
            &JsValue::from_str("facingMode"),
            &JsValue::from_str("environment"),
        )
        .map_err(js_err)?;
        constraints.set_video(&video_opts.into());

        let media = window
            .navigator()
            .media_devices()
            .map_err(|_| "camera not available (needs HTTPS or localhost)")?
            .get_user_media_with_constraints(&constraints)
            .map_err(js_err)?;
        let stream: web_sys::MediaStream = JsFuture::from(media)
            .await
            .map_err(|e| format!("camera permission denied? {}", js_err(e)))?
            .dyn_into()
            .map_err(|_| "not a MediaStream")?;
        video.set_src_object(Some(&stream));
        let _ = JsFuture::from(video.play().map_err(js_err)?).await;

        let canvas: web_sys::HtmlCanvasElement = document
            .create_element("canvas")
            .map_err(js_err)?
            .dyn_into()
            .map_err(|_| "not a canvas")?;
        let opts = js_sys::Object::new();
        js_sys::Reflect::set(
            &opts,
            &JsValue::from_str("willReadFrequently"),
            &JsValue::TRUE,
        )
        .map_err(js_err)?;
        let ctx: web_sys::CanvasRenderingContext2d = canvas
            .get_context_with_context_options("2d", &opts)
            .map_err(js_err)?
            .ok_or("no 2d context")?
            .dyn_into()
            .map_err(|_| "not a 2d context")?;

        let align_canvas: web_sys::HtmlCanvasElement = document
            .create_element("canvas")
            .map_err(js_err)?
            .dyn_into()
            .map_err(|_| "not a canvas")?;
        align_canvas.set_width(ALIGN_SIZE);
        align_canvas.set_height(ALIGN_SIZE);
        let aopts = js_sys::Object::new();
        js_sys::Reflect::set(
            &aopts,
            &JsValue::from_str("willReadFrequently"),
            &JsValue::TRUE,
        )
        .map_err(js_err)?;
        let align_ctx: web_sys::CanvasRenderingContext2d = align_canvas
            .get_context_with_context_options("2d", &aopts)
            .map_err(js_err)?
            .ok_or("no 2d context")?
            .dyn_into()
            .map_err(|_| "not a 2d context")?;

        Ok(Camera {
            video,
            canvas,
            ctx,
            align_canvas,
            align_ctx,
            stream,
        })
    }

    /// Detect how far the sticker grid sits from the guide square, in
    /// working-canvas pixels: downscale the square into the alignment
    /// canvas and run the projection matcher. Returns (dx, dy,
    /// confidence); apply the offset only when confidence is high.
    pub fn grid_align(&self) -> Option<(f32, f32, f32)> {
        let (w, h) = self.canvas_dims();
        if w == 0 || h == 0 {
            return None;
        }
        let side = 0.6 * f64::from(w.min(h));
        let left = (f64::from(w) - side) / 2.0;
        let top = (f64::from(h) - side) / 2.0;
        self.align_ctx
            .draw_image_with_html_canvas_element_and_sw_and_sh_and_dx_and_dy_and_dw_and_dh(
                &self.canvas,
                left,
                top,
                side,
                side,
                0.0,
                0.0,
                f64::from(ALIGN_SIZE),
                f64::from(ALIGN_SIZE),
            )
            .ok()?;
        let data = self
            .align_ctx
            .get_image_data(0.0, 0.0, f64::from(ALIGN_SIZE), f64::from(ALIGN_SIZE))
            .ok()?;
        let rgba = data.data().to_vec();
        let (dx, dy, conf) =
            cube_vision::grid_offset(&rgba, ALIGN_SIZE as usize, ALIGN_SIZE as usize);
        let scale = side / f64::from(ALIGN_SIZE);
        Some(((dx as f64 * scale) as f32, (dy as f64 * scale) as f32, conf))
    }

    pub fn ready(&self) -> bool {
        self.video.ready_state() >= 2 && self.video.video_width() > 0
    }

    pub fn dims(&self) -> (u32, u32) {
        (self.video.video_width(), self.video.video_height())
    }

    pub fn video(&self) -> &web_sys::HtmlVideoElement {
        &self.video
    }

    pub fn canvas(&self) -> &web_sys::HtmlCanvasElement {
        &self.canvas
    }

    /// Draw the current video frame onto the working canvas. Call once per
    /// UI frame; preview and sampling both read this canvas afterwards.
    pub fn draw_frame(&self) -> bool {
        if !self.ready() {
            return false;
        }
        let (vw, vh) = self.dims();
        let scale = f64::from(SAMPLE_WIDTH) / f64::from(vw);
        let (w, h) = (SAMPLE_WIDTH, (f64::from(vh) * scale) as u32);
        if self.canvas.width() != w || self.canvas.height() != h {
            self.canvas.set_width(w);
            self.canvas.set_height(h);
        }
        self.ctx
            .draw_image_with_html_video_element_and_dw_and_dh(
                &self.video,
                0.0,
                0.0,
                f64::from(w),
                f64::from(h),
            )
            .is_ok()
    }

    pub fn canvas_dims(&self) -> (u32, u32) {
        (self.canvas.width(), self.canvas.height())
    }

    /// Read the nine WHOLE grid cells (with a small inset against the
    /// grid lines) from the current frame. The grid square is centered,
    /// side = 0.6 * min(video dimensions), matching the overlay. Cell
    /// order is remapped by `rotation` so index 0 is the overlay's
    /// top-left. Full cells (not center points) feed the color-VOTING
    /// classifier, which survives misalignment.
    pub fn sample_cells(
        &self,
        rotation: u8,
        offset: (f32, f32),
    ) -> Option<[(Vec<u8>, usize, usize); 9]> {
        let (w, h) = self.canvas_dims();
        if w == 0 || h == 0 {
            return None;
        }
        let side = 0.6 * f64::from(w.min(h));
        // Grid auto-alignment: the whole sampled square follows the
        // detected sticker grid (clamped inside the frame).
        let max_off = side / 6.0;
        let dx = f64::from(offset.0).clamp(-max_off, max_off);
        let dy = f64::from(offset.1).clamp(-max_off, max_off);
        let left = ((f64::from(w) - side) / 2.0 + dx).clamp(0.0, f64::from(w) - side);
        let top = ((f64::from(h) - side) / 2.0 + dy).clamp(0.0, f64::from(h) - side);
        let cell = side / 3.0;
        let inset = cell * 0.14; // keep clear of the painted grid lines

        let mut out: Vec<(Vec<u8>, usize, usize)> = Vec::with_capacity(9);
        for display_row in 0..3usize {
            for display_col in 0..3usize {
                let (mut row, mut col) = (display_row, display_col);
                for _ in 0..(rotation % 4) {
                    let (r, c) = (row, col);
                    (row, col) = (2 - c, r);
                }
                let x = left + col as f64 * cell + inset;
                let y = top + row as f64 * cell + inset;
                let cw = (cell - 2.0 * inset).max(4.0);
                let data = self.ctx.get_image_data(x, y, cw, cw).ok()?;
                let rgba = data.data().to_vec();
                let px = (rgba.len() / 4) as f64;
                let side_px = px.sqrt().round() as usize;
                if side_px * side_px * 4 != rgba.len() {
                    // Non-square return (edge clamp); recompute dims.
                    let wpx = cw.floor() as usize;
                    let hpx = rgba.len() / 4 / wpx.max(1);
                    out.push((rgba, wpx, hpx));
                } else {
                    out.push((rgba, side_px, side_px));
                }
            }
        }
        out.try_into().ok()
    }

    /// The entire current sampling canvas (drawn at SAMPLE_WIDTH), for
    /// diagnostics uploads: lets us see exactly where the grid sampled
    /// relative to the cube.
    pub fn frame_rgba(&self) -> Option<(Vec<u8>, usize, usize)> {
        let w = self.canvas.width() as usize;
        let h = self.canvas.height() as usize;
        if w == 0 || h == 0 {
            return None;
        }
        let data = self
            .ctx
            .get_image_data(0.0, 0.0, w as f64, h as f64)
            .ok()?;
        Some((data.data().to_vec(), w, h))
    }

    pub fn stop(&self) {
        for track in self.stream.get_tracks().iter() {
            if let Ok(track) = track.dyn_into::<web_sys::MediaStreamTrack>() {
                track.stop();
            }
        }
        self.video.remove();
    }
}
