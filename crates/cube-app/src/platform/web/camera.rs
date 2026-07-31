//! getUserMedia camera glue. The <video> element stays hidden in the DOM
//! (1px, opacity 0 — display:none would stop playback in some browsers);
//! the visible preview is a zero-copy GPU texture drawn by the scan screen.
//! Pixel readback happens only for the nine small classification patches,
//! through a small willReadFrequently 2d canvas.

use cube_vision::Oklab;
use eframe::wasm_bindgen::{JsCast as _, JsValue};
use wasm_bindgen_futures::JsFuture;

pub struct Camera {
    video: web_sys::HtmlVideoElement,
    canvas: web_sys::HtmlCanvasElement,
    ctx: web_sys::CanvasRenderingContext2d,
    stream: web_sys::MediaStream,
}

/// Sampling resolution: frames are downscaled to this width before patch
/// reads (cheap, and plenty for 3x3 color classification).
const SAMPLE_WIDTH: u32 = 480;
/// Patch size in sample-canvas pixels.
const PATCH: u32 = 20;

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

        Ok(Camera {
            video,
            canvas,
            ctx,
            stream,
        })
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

    /// Read the nine grid patches from the current frame. The grid square
    /// is centered, with side = 0.6 * min(video dimensions) — matching the
    /// overlay the scan screen draws.
    pub fn sample_patches(&self) -> Option<[Oklab; 9]> {
        if !self.ready() {
            return None;
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
            .ok()?;

        let side = 0.6 * f64::from(w.min(h));
        let left = (f64::from(w) - side) / 2.0;
        let top = (f64::from(h) - side) / 2.0;
        let cell = side / 3.0;

        let mut out = [Oklab::default(); 9];
        for row in 0..3 {
            for col in 0..3 {
                let cx = left + (f64::from(col) + 0.5) * cell;
                let cy = top + (f64::from(row) + 0.5) * cell;
                let data = self
                    .ctx
                    .get_image_data(
                        cx - f64::from(PATCH) / 2.0,
                        cy - f64::from(PATCH) / 2.0,
                        f64::from(PATCH),
                        f64::from(PATCH),
                    )
                    .ok()?;
                let rgba = data.data();
                out[(row * 3 + col) as usize] =
                    cube_vision::srgb_patch_to_oklab(&rgba, PATCH as usize, PATCH as usize);
            }
        }
        Some(out)
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
