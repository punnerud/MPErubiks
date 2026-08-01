//! Camera scan flow (browser only), photo-first and centered:
//!
//! - fullscreen preview (rotated upright on portrait phones), a plain 3x3
//!   grid mid-screen and a thin progress line that fills while you hold
//!   still — NO buttons, capture is automatic
//! - after each auto-snap the detected colors flash briefly in the grid
//! - a mini 3D cube (top center) shows scan progress — captured sides
//!   green, the side to show next blue — and ANIMATES the rotation you
//!   should perform between captures, on loop
//! - colors are assigned from centers after all six sides, so the cube can
//!   be held any way; a small "type it in" fallback sits at the bottom

use crate::app::{AsyncMsg, RubiksApp, Screen};
use crate::i18n::TextKey;
use crate::platform::web::camera::Camera;
use crate::widgets::cube_view::CubeView;
use cube_core::{Alg, Face, FaceletCube};
use cube_render::{MoveAnimator, OrbitCamera};
use cube_vision::{vote_cell, vote_histogram, Calibration, Classified};
use egui::{Color32, Pos2, Rect, RichText, Sense, Stroke, StrokeKind, Ui, Vec2};

/// Capture order (Morten's hand sequence: left, left, tilt-AWAY, left,
/// left) and the face each capture becomes.
const ORDER: [Face; 6] = [Face::F, Face::R, Face::B, Face::D, Face::L, Face::U];

/// The physical rotation the user performs BEFORE capture k (mini-cube
/// demo loops this until the hold-steady snap fires). "Turn left" is y
/// (the right side comes to the front); "tilt away" is x (the top tips
/// away from you, the bottom comes to the camera).
const MINI_ALGS: [&str; 6] = ["", "y", "y", "x", "y", "y"];

/// Camera-grid position (row-major) -> facelet offset within the face.
/// Derived from the orientation matrices of the sequence above: F/R/B
/// land row-major straight; D appears 180° rotated, L 90°. U lands
/// row-major straight too — after y y x y y the camera sees U with B at
/// the top and L at the left, exactly the Kociemba reference view
/// (verified both by hand-tracked orientation and by 3D-coordinate
/// simulation; the old vertical flip came from the retired tilt-toward
/// sequence and made U-corner pieces physically impossible).
const CAPTURE_GRID_TO_FACELET: [[u8; 9]; 6] = [
    [0, 1, 2, 3, 4, 5, 6, 7, 8],       // F
    [0, 1, 2, 3, 4, 5, 6, 7, 8],       // R
    [0, 1, 2, 3, 4, 5, 6, 7, 8],       // B
    [8, 7, 6, 5, 4, 3, 2, 1, 0],       // D (180°)
    [6, 3, 0, 7, 4, 1, 8, 5, 2],       // L (90°)
    [0, 1, 2, 3, 4, 5, 6, 7, 8],       // U
];

/// Continuous stable ticks (10 Hz) required before the auto-snap fires:
/// 1.5 s of calm. Tick-to-tick class agreement doubles as blur detection
/// — motion blur flickers the classification and resets the counter —
/// and the evidence histograms are AVERAGED over the whole stable window,
/// so the snap is many frames double-checking each other, not one lucky
/// frame.
const SNAP_TICKS: u8 = 15;

pub enum CameraState {
    Requesting,
    Ready(Camera),
    Error(String),
}

struct PreviewTex {
    texture: wgpu::Texture,
    id: egui::TextureId,
    size: (u32, u32),
}

pub struct ScanScreen {
    camera: CameraState,
    preview: Option<PreviewTex>,
    /// Voted class per cell for each captured face (display order).
    captured: [Option<[Option<u8>; 9]>; 6],
    /// Retained evidence: full vote histogram per cell per face — the
    /// constraint resolver ranks candidates from this after capture.
    evidence: [Option<[[f32; 6]; 9]>; 6],
    /// Histogram accumulator over the current stable window (sum, ticks).
    evid_sum: [[f32; 6]; 9],
    evid_n: f32,
    face_idx: usize,
    live: [Classified; 9],
    /// Raw cell buffers from the latest sampling tick (for upload).
    last_cells: Option<[(Vec<u8>, usize, usize); 9]>,
    /// Classes of the face just captured: the camera must see a DIFFERENT
    /// side before the auto-snap re-arms (no double-captures of one side).
    last_captured: Option<[Option<u8>; 9]>,
    stable_ticks: u8,
    /// Smoothed grid auto-fit (canvas px offset + per-axis scale) and its
    /// confidence: sampling follows the cube's actual position and size,
    /// invisibly — the painted grid stays fixed.
    grid_fit: (f32, f32, f32, f32),
    grid_conf: f32,
    last_sample: f64,
    cal: Calibration,
    /// Preview rotation in CW quarter turns; 255 = auto-guess. User can
    /// cycle it with the on-screen button (sensor orientation varies by
    /// device); persisted as the "cam_rot" setting.
    rotation: u8,
    /// Post-capture color flash: (show until, detected class per cell).
    flash: Option<(f64, [Option<u8>; 9])>,
    // Mini 3D rotation guide.
    mini_cube: FaceletCube,
    mini_base: FaceletCube,
    mini_anim: MoveAnimator,
    mini_orbit: OrbitCamera,
    mini_last_loop: f64,
}

impl ScanScreen {
    pub fn new(app: &RubiksApp, ctx: &egui::Context) -> Self {
        let tx = app.tx.clone();
        let ctx = ctx.clone();
        wasm_bindgen_futures::spawn_local(async move {
            let result = Camera::open().await;
            let _ = tx.send(AsyncMsg::CameraReady(result));
            ctx.request_repaint();
        });
        let mut mini_anim = MoveAnimator::default();
        mini_anim.secs_per_quarter = 0.5; // calm, readable demo
        ScanScreen {
            camera: CameraState::Requesting,
            preview: None,
            captured: [None; 6],
            evidence: [None; 6],
            evid_sum: [[0.0; 6]; 9],
            evid_n: 0.0,
            face_idx: 0,
            live: [Classified {
                color: None,
                confidence: 0.0,
            }; 9],
            last_cells: None,
            last_captured: None,
            stable_ticks: 0,
            grid_fit: (0.0, 0.0, 1.0, 1.0),
            grid_conf: 0.0,
            last_sample: 0.0,
            cal: Calibration::default_stickers(),
            rotation: app
                .store
                .as_ref()
                // Fresh key: pre-canvas-source rotations are obsolete.
                .and_then(|s| s.setting("cam_rot2").ok().flatten())
                .and_then(|v| v.parse().ok())
                .unwrap_or(0),
            flash: None,
            mini_cube: FaceletCube::SOLVED,
            mini_base: FaceletCube::SOLVED,
            mini_anim,
            mini_orbit: OrbitCamera::default(),
            mini_last_loop: 0.0,
        }
    }

    pub fn set_camera(&mut self, result: Result<Camera, String>) {
        self.camera = match result {
            Ok(c) => CameraState::Ready(c),
            Err(e) => CameraState::Error(e),
        };
    }

    pub fn stop(&self) {
        if let CameraState::Ready(cam) = &self.camera {
            cam.stop();
        }
    }
}

pub fn show(app: &mut RubiksApp, ui: &mut Ui, frame: &mut eframe::Frame) {
    super::play::top_bar(app, ui);
    let Screen::Scan(mut screen) = std::mem::replace(&mut app.screen, Screen::Menu) else {
        return;
    };
    let mut next: Option<Screen> = None;

    match &screen.camera {
        CameraState::Requesting => {
            ui.vertical_centered(|ui| {
                ui.add_space(40.0);
                ui.spinner();
                ui.label(app.t(TextKey::CameraStarting));
            });
        }
        CameraState::Error(e) => {
            ui.vertical_centered(|ui| {
                ui.add_space(40.0);
                ui.colored_label(Color32::LIGHT_RED, RichText::new("📷 ✘").size(40.0));
                ui.label(e.clone());
                if big_button(ui, app.t(TextKey::EnterManually), Color32::from_rgb(0x2A, 0x5C, 0xC2))
                {
                    next = Some(Screen::Solve(super::solve::SolveScreen::new_input(
                        FaceletCube::SOLVED,
                    )));
                }
            });
        }
        CameraState::Ready(_) => {
            scan_ui(app, ui, frame, &mut screen, &mut next);
        }
    }

    if let Some(next_screen) = next {
        screen.stop();
        app.screen = next_screen;
    } else {
        app.screen = Screen::Scan(screen);
    }
}

fn scan_ui(
    app: &mut RubiksApp,
    ui: &mut Ui,
    frame: &mut eframe::Frame,
    screen: &mut ScanScreen,
    next: &mut Option<Screen>,
) {
    let (cam_ready, dims, video) = {
        let CameraState::Ready(cam) = &screen.camera else {
            return;
        };
        (cam.ready(), cam.dims(), cam.video().clone())
    };
    let now = ui.input(|i| i.time);
    ui.ctx().request_repaint();

    let avail = ui.available_rect_before_wrap();
    // The working canvas is the single source for preview AND sampling,
    // and canvas drawImage applies platform orientation — so the default
    // is no extra rotation. The ↻ button remains for odd devices.
    let rotation = screen.rotation % 4;
    // Refresh the canvas with the current frame (preview + sampling read it).
    let (canvas, canvas_dims) = {
        let CameraState::Ready(cam) = &screen.camera else {
            return;
        };
        cam.draw_frame();
        (cam.canvas().clone(), cam.canvas_dims())
    };
    let _ = (dims, video);

    // --- classification tick at 10 Hz (internal only: drives auto-snap) ---
    let in_flash = screen.flash.is_some_and(|(until, _)| now < until);
    if cam_ready && !in_flash && screen.face_idx < 6 && now - screen.last_sample > 0.1 {
        screen.last_sample = now;
        if let CameraState::Ready(cam) = &screen.camera {
            if let Some((dx, dy, sx, sy, conf)) = cam.grid_align() {
                screen.grid_conf = conf;
                let target = if conf > 0.4 {
                    (dx, dy, sx, sy)
                } else {
                    (0.0, 0.0, 1.0, 1.0)
                };
                // Smooth toward the detected grid: steady, not jittery.
                screen.grid_fit.0 += (target.0 - screen.grid_fit.0) * 0.35;
                screen.grid_fit.1 += (target.1 - screen.grid_fit.1) * 0.35;
                screen.grid_fit.2 += (target.2 - screen.grid_fit.2) * 0.35;
                screen.grid_fit.3 += (target.3 - screen.grid_fit.3) * 0.35;
            }
        }
        let cells = match &screen.camera {
            CameraState::Ready(cam) => cam.sample_cells(rotation, screen.grid_fit),
            _ => None,
        };
        if let Some(cells) = cells {
            // Color VOTING per whole cell: a recurring color >5% with a
            // clear margin wins — robust to misalignment and background.
            let live: [Classified; 9] = core::array::from_fn(|i| {
                let (buf, w, h) = &cells[i];
                vote_cell(buf, *w, *h, &screen.cal)
            });
            // Seven of nine suffice: the constraint resolver analyses
            // the uncertain rest from retained evidence.
            let all_detected = live.iter().filter(|c| c.color.is_some()).count() >= 7;
            // Tolerate up to two flickering cells per tick: demanding
            // all nine identical for 1.5s is unreachable hand-held (one
            // flicker reset the whole window -> capture never fired).
            // The averaged evidence smooths the flicker out anyway.
            let same = screen
                .live
                .iter()
                .zip(live.iter())
                .filter(|(a, b)| a.color != b.color)
                .count()
                <= 2;
            // Re-arm only once the camera sees a genuinely DIFFERENT side
            // than the one just captured (>=3 cells differ) — otherwise
            // one face auto-captures six times before anyone can rotate.
            let rotated_away = screen.last_captured.map_or(true, |prev| {
                prev.iter()
                    .zip(live.iter())
                    .filter(|(p, l)| **p != l.color)
                    .count()
                    >= 3
            });
            let aligned = screen.grid_conf > 0.4;
            if all_detected && same && rotated_away && aligned {
                screen.stable_ticks = screen.stable_ticks.saturating_add(1);
                for (i, (buf, w, h)) in cells.iter().enumerate() {
                    let hist = vote_histogram(buf, *w, *h, &screen.cal);
                    for c in 0..6 {
                        screen.evid_sum[i][c] += hist[c];
                    }
                }
                screen.evid_n += 1.0;
            } else {
                screen.stable_ticks = 0;
                screen.evid_sum = [[0.0; 6]; 9];
                screen.evid_n = 0.0;
            }
            screen.live = live;
            screen.last_cells = Some(cells);
            if screen.stable_ticks >= SNAP_TICKS {
                capture(screen, now, "auto");
            }
        }
    }
    if screen.flash.is_some_and(|(until, _)| now >= until) {
        screen.flash = None;
    }

    // --- preview texture from the working canvas ---
    if let (Some(rs), true) = (frame.wgpu_render_state(), cam_ready) {
        let dims = canvas_dims;
        let recreate = screen.preview.as_ref().map_or(true, |p| p.size != dims);
        if recreate {
            let texture = rs.device.create_texture(&wgpu::TextureDescriptor {
                label: Some("camera preview"),
                size: wgpu::Extent3d {
                    width: dims.0,
                    height: dims.1,
                    depth_or_array_layers: 1,
                },
                mip_level_count: 1,
                sample_count: 1,
                dimension: wgpu::TextureDimension::D2,
                format: wgpu::TextureFormat::Rgba8UnormSrgb,
                usage: wgpu::TextureUsages::COPY_DST
                    | wgpu::TextureUsages::RENDER_ATTACHMENT
                    | wgpu::TextureUsages::TEXTURE_BINDING,
                view_formats: &[],
            });
            let view = texture.create_view(&Default::default());
            let id = rs.renderer.write().register_native_texture(
                &rs.device,
                &view,
                wgpu::FilterMode::Linear,
            );
            screen.preview = Some(PreviewTex {
                texture,
                id,
                size: dims,
            });
        }
        if let Some(preview) = &screen.preview {
            rs.queue.copy_external_image_to_texture(
                &wgpu::CopyExternalImageSourceInfo {
                    source: wgpu::ExternalImageSource::HTMLCanvasElement(canvas.clone()),
                    origin: wgpu::Origin2d::ZERO,
                    flip_y: false,
                },
                wgpu::CopyExternalImageDestInfo {
                    texture: &preview.texture,
                    mip_level: 0,
                    origin: wgpu::Origin3d::ZERO,
                    aspect: wgpu::TextureAspect::All,
                    color_space: wgpu::PredefinedColorSpace::Srgb,
                    premultiplied_alpha: false,
                },
                wgpu::Extent3d {
                    width: preview.size.0,
                    height: preview.size.1,
                    depth_or_array_layers: 1,
                },
            );
        }
    }

    // --- fullscreen preview (aspect-fit, upright) ---
    let (outer, _) = ui.allocate_exact_size(avail.size(), Sense::hover());
    if let Some(preview) = &screen.preview {
        let (vw, vh) = (preview.size.0 as f32, preview.size.1 as f32);
        let (sw, sh) = if rotation % 2 == 1 { (vh, vw) } else { (vw, vh) };
        let scale = (outer.width() / sw).min(outer.height() / sh);
        let shown = Vec2::new(sw * scale, sh * scale);
        let rect = Rect::from_center_size(outer.center(), shown);
        draw_preview(ui, preview.id, rect, rotation);
        // The grid GRIPS the cube invisibly (sampling follows the
        // detected offset); the painted square stays fixed and calm —
        // a moving frame reads as jitter, not help.
        draw_overlay(ui, rect, screen, now);

        // Rotation cycle button (top-right of the preview): sensor
        // orientation differs per device — one tap fixes it, remembered.
        let btn = Rect::from_center_size(
            Pos2::new(rect.right() - 34.0, rect.top() + 34.0),
            Vec2::splat(48.0),
        );
        let resp = ui.interact(btn, ui.id().with("rot"), Sense::click());
        ui.painter()
            .rect_filled(btn, 12.0, Color32::from_black_alpha(150));
        crate::widgets::icons::draw_reset(ui.painter(), btn.shrink(12.0));
        if resp.clicked() {
            screen.rotation = (rotation + 1) % 4;
            screen.stable_ticks = 0;
            screen.evid_sum = [[0.0; 6]; 9];
            screen.evid_n = 0.0;
            if let Some(store) = &app.store {
                let _ = store.set_setting("cam_rot2", &screen.rotation.to_string());
                crate::persist::persist(store);
            }
        }
    }

    // --- mini rotation guide (top center, overlaid) ---
    mini_guide(app, ui, screen, outer, now);

    // --- bottom row: restart | force-capture | type it in ---
    let slot = |i: i32| {
        Rect::from_center_size(
            Pos2::new(outer.center().x + i as f32 * 150.0, outer.bottom() - 34.0),
            Vec2::new(136.0, 42.0),
        )
    };
    let small_button = |ui: &Ui, rect: Rect, id: &str, text: &str, accent: bool| -> bool {
        let resp = ui.interact(rect, ui.id().with(id), Sense::click());
        let fill = if accent {
            Color32::from_rgba_unmultiplied(0x1E, 0x88, 0x50, 210)
        } else {
            Color32::from_black_alpha(140)
        };
        ui.painter().rect_filled(rect, 10.0, fill);
        ui.painter().text(
            rect.center(),
            egui::Align2::CENTER_CENTER,
            text,
            egui::FontId::proportional(16.0),
            Color32::from_gray(230),
        );
        resp.clicked()
    };
    // Restart the whole scan from side one.
    if small_button(ui, slot(-1), "restart", app.t(TextKey::Reset), false) {
        screen.captured = [None; 6];
        screen.evidence = [None; 6];
        screen.evid_sum = [[0.0; 6]; 9];
        screen.evid_n = 0.0;
        screen.face_idx = 0;
        screen.stable_ticks = 0;
        screen.flash = None;
        screen.mini_base = FaceletCube::SOLVED;
        screen.mini_cube = FaceletCube::SOLVED;
        screen.mini_anim.clear();
        screen.last_captured = None;
    }
    // Force-capture when auto won't bite (tricky stickers).
    if small_button(ui, slot(0), "force", app.t(TextKey::Capture), true) && !in_flash {
        capture(screen, now, "manual");
        // Manual snaps also ship the WHOLE frame: ground truth for where
        // the grid sampled relative to the cube.
        if let CameraState::Ready(cam) = &screen.camera {
            if let Some((buf, w, h)) = cam.frame_rgba() {
                let body = format!(
                    "{{\"source\":\"frame\",\"rotation\":{rotation},\"cells\":[{{\"w\":{w},\"h\":{h},\"rgba_hex\":\"{}\"}}]}}",
                    crate::platform::web::hex_encode(&buf)
                );
                crate::platform::web::post_json_forget("upload", body);
            }
        }
    }
    if small_button(ui, slot(1), "manual", app.t(TextKey::EnterManually), false) {
        *next = Some(Screen::Solve(super::solve::SolveScreen::new_input(
            FaceletCube::SOLVED,
        )));
    }

    // --- all six captured: constraint-resolve and hand off to review ---
    if screen.face_idx >= 6 && screen.flash.is_none() {
        let shares = face_shares(screen);
        let state = match cube_solver::resolve_scan(&shares) {
            // Trust the resolver only while it stays NEAR the evidence: a
            // wrong-way rotation scrambles whole faces, and a legal cube
            // found by changing many cells is legal-but-not-yours. Show
            // the raw reading in the review net instead in that case.
            Ok(resolved) => {
                let corrections = (0..54)
                    .filter(|&i| {
                        let am = (0..6)
                            .max_by(|&a, &b| shares[i][a].partial_cmp(&shares[i][b]).unwrap())
                            .unwrap();
                        resolved.0[i] as usize != am
                    })
                    .count();
                if corrections <= 8 {
                    resolved
                } else {
                    assemble(screen)
                }
            }
            // Unresolvable even after analysis: show the argmax cube in
            // the review net; validation will point at the problem there.
            Err(_) => assemble(screen),
        };
        // Faces were labeled by CAPTURE ORDER; rotate + relabel so every
        // color sits on its standard face — the 3D cube and the guide
        // then speak the physical cube's colors (red shows red), instead
        // of the capture positions' (red-first showed green).
        let capture_class = capture_classes(screen);
        let mut class_of_face = [0usize; 6];
        for (k, &face) in ORDER.iter().enumerate() {
            class_of_face[face as usize] = capture_class[k];
        }
        let state = cube_solver::relabel_to_standard(&state, &class_of_face).unwrap_or(state);
        *next = Some(Screen::Solve(super::solve::SolveScreen::new_input(state)));
    }
}

/// Palette class of each capture's center, by OPTIMAL assignment over
/// the retained evidence (survives unreadable centers and duplicate
/// center votes).
fn capture_classes(screen: &ScanScreen) -> [usize; 6] {
    let center_hists: [[f32; 6]; 6] = core::array::from_fn(|k| {
        screen.evidence[k].map(|h| h[4]).unwrap_or([0.0; 6])
    });
    cube_solver::assign_classes(&center_hists)
}

/// Evidence histograms mapped into FACE space for the resolver: palette
/// class -> face via the voted centers (capture order fallback).
fn face_shares(screen: &ScanScreen) -> cube_solver::Shares {
    let capture_class = capture_classes(screen);
    let mut class_to_face: [Option<Face>; 6] = [None; 6];
    for (k, &face) in ORDER.iter().enumerate() {
        class_to_face[capture_class[k]] = Some(face);
    }
    let mut shares: cube_solver::Shares = [[0.0; 6]; 54];
    for (k, &face) in ORDER.iter().enumerate() {
        let Some(hists) = screen.evidence[k] else {
            continue;
        };
        for (grid_pos, &facelet_off) in CAPTURE_GRID_TO_FACELET[k].iter().enumerate() {
            let facelet = face as usize * 9 + facelet_off as usize;
            for class in 0..6 {
                if let Some(target) = class_to_face[class] {
                    shares[facelet][target as usize] += hists[grid_pos][class];
                }
            }
        }
    }
    shares
}

fn capture(screen: &mut ScanScreen, now: f64, source: &str) {
    if screen.face_idx >= 6 {
        return;
    }
    {
        let classes: [Option<u8>; 9] = core::array::from_fn(|i| screen.live[i].color);
        // Retain the evidence for constraint resolution: the average over
        // the stable window when one exists (auto-snap), else the last
        // frame (manual snap).
        if screen.evid_n > 0.0 {
            let n = screen.evid_n;
            screen.evidence[screen.face_idx] =
                Some(core::array::from_fn(|i| screen.evid_sum[i].map(|v| v / n)));
        } else if let Some(cells) = &screen.last_cells {
            let hists: [[f32; 6]; 9] = core::array::from_fn(|i| {
                let (buf, w, h) = &cells[i];
                vote_histogram(buf, *w, *h, &screen.cal)
            });
            screen.evidence[screen.face_idx] = Some(hists);
        }
        screen.evid_sum = [[0.0; 6]; 9];
        screen.evid_n = 0.0;
        // Flash what was detected in the grid for a moment.
        screen.flash = Some((now + 0.9, classes));
        screen.captured[screen.face_idx] = Some(classes);
        // Bake the rotation the user just performed into the mini guide.
        if let Ok(alg) = Alg::parse(MINI_ALGS[screen.face_idx]) {
            screen.mini_base.apply_alg(&alg);
        }
        screen.mini_cube = screen.mini_base;
        screen.mini_anim.clear();
        screen.face_idx += 1;
        screen.stable_ticks = 0;
        screen.last_captured = Some(classes);
        upload_capture(screen, source, &classes);
    }
}

/// Ship the capture (cell images + votes) to the server: real-world
/// training data for tuning the matcher.
fn upload_capture(screen: &ScanScreen, source: &str, classes: &[Option<u8>; 9]) {
    let Some(cells) = &screen.last_cells else {
        return;
    };
    let cells_json: Vec<String> = cells
        .iter()
        .map(|(buf, w, h)| {
            format!(
                "{{\"w\":{w},\"h\":{h},\"rgba_hex\":\"{}\"}}",
                crate::platform::web::hex_encode(buf)
            )
        })
        .collect();
    let votes: Vec<String> = classes
        .iter()
        .map(|c| c.map(|v| v.to_string()).unwrap_or_else(|| "null".into()))
        .collect();
    let body = format!(
        "{{\"source\":\"{source}\",\"face_idx\":{},\"votes\":[{}],\"cells\":[{}]}}",
        screen.face_idx.saturating_sub(1),
        votes.join(","),
        cells_json.join(",")
    );
    crate::platform::web::post_json_forget("upload", body);
}

/// Rebuild the facelet cube from the voted classes: a class maps to the
/// face captured with that color's position in the ORDER (the class index
/// IS the palette color; centers map colors to faces via which capture
/// they appeared as the center of — falling back to palette order).
fn assemble(screen: &ScanScreen) -> FaceletCube {
    // Which palette class did each capture's CENTER vote? That defines
    // the class -> face mapping (kid can hold the cube any way). Missing
    // centers (dark/logo) fall back to identity via capture order.
    let mut class_to_face: [Option<Face>; 6] = [None; 6];
    for (k, &face) in ORDER.iter().enumerate() {
        if let Some(classes) = screen.captured[k] {
            if let Some(center_class) = classes[4] {
                if class_to_face[center_class as usize].is_none() {
                    class_to_face[center_class as usize] = Some(face);
                }
            }
        }
    }
    let mut cube = FaceletCube::SOLVED;
    for (k, &face) in ORDER.iter().enumerate() {
        let classes = screen.captured[k].expect("captured");
        for (grid_pos, &facelet_off) in CAPTURE_GRID_TO_FACELET[k].iter().enumerate() {
            let mapped = classes[grid_pos]
                .and_then(|class| class_to_face[class as usize])
                .unwrap_or(face);
            cube.0[face as usize * 9 + facelet_off as usize] = mapped;
        }
        // The center DEFINES the face in this flow (capture order), no
        // matter what the votes said — guarantees six distinct centers
        // so review/validation always start from a sane frame.
        cube.0[face as usize * 9 + 4] = face;
    }
    cube
}

/// Textured quad turned `rotation` CW quarter-turns so any device's
/// sensor orientation can be displayed upright.
fn draw_preview(ui: &Ui, id: egui::TextureId, rect: Rect, rotation: u8) {
    let mut mesh = egui::Mesh::with_texture(id);
    let corners = [
        rect.left_top(),
        rect.right_top(),
        rect.right_bottom(),
        rect.left_bottom(),
    ];
    let base = [(0.0, 0.0), (1.0, 0.0), (1.0, 1.0), (0.0, 1.0)];
    let r = rotation as usize % 4;
    let uvs: [(f32, f32); 4] = core::array::from_fn(|i| base[(i + 4 - r) % 4]);
    for (pos, uv) in corners.iter().zip(uvs.iter()) {
        mesh.vertices.push(egui::epaint::Vertex {
            pos: *pos,
            uv: Pos2::new(uv.0, uv.1),
            color: Color32::WHITE,
        });
    }
    mesh.indices.extend([0, 1, 2, 0, 2, 3]);
    ui.painter().add(egui::Shape::mesh(mesh));
}

fn draw_overlay(ui: &Ui, rect: Rect, screen: &ScanScreen, now: f64) {
    let p = ui.painter();
    let side = rect.width().min(rect.height()) * 0.6;
    let square = Rect::from_center_size(rect.center(), Vec2::splat(side));

    // Dim outside the guide square.
    for r in [
        Rect::from_min_max(rect.min, Pos2::new(rect.max.x, square.min.y)),
        Rect::from_min_max(Pos2::new(rect.min.x, square.max.y), rect.max),
        Rect::from_min_max(Pos2::new(rect.min.x, square.min.y), Pos2::new(square.min.x, square.max.y)),
        Rect::from_min_max(Pos2::new(square.max.x, square.min.y), Pos2::new(rect.max.x, square.max.y)),
    ] {
        p.rect_filled(r, 0.0, Color32::from_black_alpha(110));
    }

    let cell = side / 3.0;
    // BIG direction hint while waiting for the next side (reinforces the
    // mini-cube demo; rotating the wrong way is the costliest mistake):
    // pulsing chevrons across the square — left for y, up-and-over for
    // the tilt-away x.
    if screen.face_idx > 0 && screen.face_idx < 6 && screen.flash.is_none() {
        let alg = MINI_ALGS[screen.face_idx];
        let pulse = (((now * 2.0).sin() * 0.5 + 0.5) * 90.0) as u8 + 50;
        let col = Color32::from_rgba_unmultiplied(0xFF, 0xFF, 0xFF, pulse);
        let a = side * 0.09;
        for i in 0..3 {
            let t = i as f32 - 1.0;
            if alg == "x" {
                // Tilt AWAY: chevrons along the top edge pointing up.
                let c = Pos2::new(square.center().x + t * a * 3.0, square.top() - a * 1.2);
                p.add(egui::Shape::convex_polygon(
                    vec![
                        Pos2::new(c.x, c.y - a * 0.7),
                        Pos2::new(c.x + a * 0.8, c.y + a * 0.7),
                        Pos2::new(c.x - a * 0.8, c.y + a * 0.7),
                    ],
                    col,
                    Stroke::NONE,
                ));
            } else {
                // Turn LEFT: chevrons along the left edge pointing left.
                let c = Pos2::new(square.left() - a * 1.2, square.center().y + t * a * 3.0);
                p.add(egui::Shape::convex_polygon(
                    vec![
                        Pos2::new(c.x - a * 0.7, c.y),
                        Pos2::new(c.x + a * 0.7, c.y - a * 0.8),
                        Pos2::new(c.x + a * 0.7, c.y + a * 0.8),
                    ],
                    col,
                    Stroke::NONE,
                ));
            }
        }
    }

    // Locked onto the sticker grid -> green lines (visual "got it!");
    // searching -> soft white.
    let grid_stroke = if screen.grid_conf > 0.4 {
        Stroke::new(2.5, Color32::from_rgba_unmultiplied(0x39, 0xD3, 0x76, 220))
    } else {
        Stroke::new(2.0, Color32::from_white_alpha(190))
    };
    for i in 0..=3 {
        let x = square.left() + i as f32 * cell;
        p.line_segment([Pos2::new(x, square.top()), Pos2::new(x, square.bottom())], grid_stroke);
        let y = square.top() + i as f32 * cell;
        p.line_segment([Pos2::new(square.left(), y), Pos2::new(square.right(), y)], grid_stroke);
    }

    // Post-capture flash: show the detected colors inside the cells.
    if let Some((until, classes)) = screen.flash {
        if now < until {
            for row in 0..3 {
                for col in 0..3 {
                    let cr = Rect::from_min_size(
                        Pos2::new(
                            square.left() + col as f32 * cell,
                            square.top() + row as f32 * cell,
                        ),
                        Vec2::splat(cell),
                    )
                    .shrink(cell * 0.12);
                    let color = classes[row * 3 + col]
                        .map(class_color)
                        .unwrap_or(Color32::from_gray(120));
                    p.rect_filled(cr, cell * 0.15, color.gamma_multiply(0.9));
                }
            }
            return;
        }
    }

    // Hold-steady progress: a thin line above the square that fills up.
    // Orange full bar = "this is the side I already have — rotate!".
    let same_side = screen.face_idx < 6
        && screen.last_captured.map_or(false, |prev| {
            prev.iter()
                .zip(screen.live.iter())
                .filter(|(p, l)| **p != l.color)
                .count()
                < 3
        });
    let t = f32::from(screen.stable_ticks.min(SNAP_TICKS)) / f32::from(SNAP_TICKS);
    let bar_bg = Rect::from_min_size(
        Pos2::new(square.left(), square.top() - 16.0),
        Vec2::new(side, 6.0),
    );
    p.rect_filled(bar_bg, 3.0, Color32::from_black_alpha(140));
    if same_side {
        p.rect_filled(bar_bg, 3.0, Color32::from_rgb(0xE0, 0x8A, 0x1E));
    } else if t > 0.0 && screen.face_idx < 6 {
        let bar = Rect::from_min_size(bar_bg.min, Vec2::new(side * t, 6.0));
        p.rect_filled(bar, 3.0, Color32::from_rgb(0x4C, 0xD9, 0x64));
    }
}

/// Mini 3D cube, top center: white cube, captured sides green, the side to
/// show next blue; loops the rotation the user should perform.
fn mini_guide(app: &mut RubiksApp, ui: &mut Ui, screen: &mut ScanScreen, outer: Rect, now: f64) {
    // Advance + loop the demo animation.
    for m in screen.mini_anim.tick(now) {
        screen.mini_cube.apply(m);
    }
    let alg_str = MINI_ALGS[screen.face_idx.min(5)];
    if screen.face_idx < 6
        && !alg_str.is_empty()
        && screen.mini_anim.is_idle()
        && now - screen.mini_last_loop > 1.6
    {
        screen.mini_cube = screen.mini_base;
        if let Ok(alg) = Alg::parse(alg_str) {
            screen.mini_anim.enqueue_all(&alg.0);
        }
        screen.mini_last_loop = now;
    }

    // Status colors by ORIGINAL face label (survives mini rotations).
    let mut palette_of = [0u32; 54];
    for (i, &label) in screen.mini_cube.0.iter().enumerate() {
        let k = ORDER.iter().position(|&f| f == label).unwrap_or(0);
        palette_of[i] = if k < screen.face_idx {
            2 // green: captured
        } else if k == screen.face_idx {
            5 // blue: show this side now
        } else {
            0 // white: later
        };
    }

    let size = 132.0;
    let pos = Pos2::new(outer.center().x - size / 2.0, outer.top() + 8.0);
    egui::Area::new(ui.id().with("mini-guide"))
        .fixed_pos(pos)
        .show(ui.ctx(), |ui| {
            let override_fn = |facelet: usize| Some(palette_of[facelet]);
            CubeView {
                cube: &screen.mini_cube,
                animator: &screen.mini_anim,
                orbit: &mut screen.mini_orbit,
                highlight: None,
                dim_others: 1.0,
                color_override: Some(&override_fn),
            }
            .show(ui, Vec2::splat(size));
            // Six progress squares under the mini cube.
            ui.horizontal(|ui| {
                ui.add_space((size - 6.0 * 18.0).max(0.0) / 2.0);
                for k in 0..6 {
                    let (rect, _) = ui.allocate_exact_size(Vec2::splat(14.0), Sense::hover());
                    let color = if k < screen.face_idx {
                        Color32::from_rgb(0x1E, 0x88, 0x50)
                    } else if k == screen.face_idx {
                        Color32::from_rgb(0x2A, 0x5C, 0xC2)
                    } else {
                        Color32::from_gray(90)
                    };
                    ui.painter().rect_filled(rect, 4.0, color);
                }
            });
        });
    let _ = app;
}

/// Default-calibration class colors (white yellow red orange green blue).
fn class_color(class: u8) -> Color32 {
    [
        Color32::from_rgb(0xF5, 0xF5, 0xF5),
        Color32::from_rgb(0xFF, 0xD5, 0x00),
        Color32::from_rgb(0xE0, 0x1B, 0x2E),
        Color32::from_rgb(0xFF, 0x61, 0x00),
        Color32::from_rgb(0x00, 0xA8, 0x60),
        Color32::from_rgb(0x0D, 0x5C, 0xC7),
    ][class as usize % 6]
}

fn big_button(ui: &mut Ui, text: &str, color: Color32) -> bool {
    let size = Vec2::new(240.0, 64.0);
    let (rect, response) = ui.allocate_exact_size(size, Sense::click());
    ui.painter().rect_filled(rect, 14.0, color);
    ui.painter().text(
        rect.center(),
        egui::Align2::CENTER_CENTER,
        text,
        egui::FontId::proportional(22.0),
        Color32::WHITE,
    );
    response.clicked()
}

// StrokeKind kept in imports for future outline needs.
#[allow(unused)]
fn _unused(_: StrokeKind) {}
