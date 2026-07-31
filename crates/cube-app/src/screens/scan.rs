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
use cube_vision::{classify_face, Calibration, Classified, Oklab};
use egui::{Color32, Pos2, Rect, RichText, Sense, Stroke, StrokeKind, Ui, Vec2};

/// Capture order (Morten's easy hand sequence: left, left, tilt-forward,
/// left, left) and the face each capture becomes.
const ORDER: [Face; 6] = [Face::F, Face::R, Face::B, Face::U, Face::L, Face::D];

/// The physical rotation the user performs BEFORE capture k (mini-cube
/// demo loops this until the hold-steady snap fires). "Turn left" is y
/// (the right side comes to the front); "tilt forward" is x' (the top
/// comes to the front).
const MINI_ALGS: [&str; 6] = ["", "y", "y", "x'", "y", "y"];

/// Camera-grid position (row-major) -> facelet offset within the face.
/// Derived from the orientation matrices of the sequence above: F/R/B/D
/// land row-major straight, U appears rotated 180°, L rotated 90°.
/// Encoded as data so a physical-cube discrepancy is a table fix.
const CAPTURE_GRID_TO_FACELET: [[u8; 9]; 6] = [
    [0, 1, 2, 3, 4, 5, 6, 7, 8],       // F
    [0, 1, 2, 3, 4, 5, 6, 7, 8],       // R
    [0, 1, 2, 3, 4, 5, 6, 7, 8],       // B
    [8, 7, 6, 5, 4, 3, 2, 1, 0],       // U (180°)
    [2, 5, 8, 1, 4, 7, 0, 3, 6],       // L (90°)
    [0, 1, 2, 3, 4, 5, 6, 7, 8],       // D
];

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
    captured: [Option<[Oklab; 9]>; 6],
    face_idx: usize,
    live: [Classified; 9],
    live_patches: Option<[Oklab; 9]>,
    stable_ticks: u8,
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
            face_idx: 0,
            live: [Classified {
                color: None,
                confidence: 0.0,
            }; 9],
            live_patches: None,
            stable_ticks: 0,
            last_sample: 0.0,
            cal: Calibration::default_stickers(),
            rotation: app
                .store
                .as_ref()
                .and_then(|s| s.setting("cam_rot").ok().flatten())
                .and_then(|v| v.parse().ok())
                .unwrap_or(255),
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
    // CW quarter turns to display upright. Auto mode uses the Screen
    // Orientation API: if frames arrive landscape while the screen is
    // rotated by `angle`, the upright correction is (90 - angle)/90
    // quarter-turns (sensors are landscape-mounted in natural portrait).
    // Safari sometimes pre-rotates frames (dims match the screen) — then
    // no correction is needed. The ↻ button overrides odd devices.
    let rotation = if screen.rotation == 255 {
        let angle = crate::platform::web::screen_angle() as i32;
        let frames_landscape = dims.0 > dims.1;
        let screen_portrait = avail.height() > avail.width();
        if frames_landscape && screen_portrait {
            (((450 - angle) % 360) / 90) as u8 % 4
        } else {
            0
        }
    } else {
        screen.rotation % 4
    };

    // --- classification tick at 10 Hz (internal only: drives auto-snap) ---
    let in_flash = screen.flash.is_some_and(|(until, _)| now < until);
    if cam_ready && !in_flash && screen.face_idx < 6 && now - screen.last_sample > 0.1 {
        screen.last_sample = now;
        let patches = match &screen.camera {
            CameraState::Ready(cam) => cam.sample_patches(rotation),
            _ => None,
        };
        if let Some(patches) = patches {
            let live = classify_face(&patches, &screen.cal);
            // Enough-of-nine, not all-of-nine: dark/odd stickers (black
            // logo centers, glare) classify as unknown and get fixed in
            // the review net — they must not block the snap forever.
            let confident = live
                .iter()
                .filter(|c| c.color.is_some() && c.confidence > 0.45)
                .count();
            let same = screen
                .live
                .iter()
                .zip(live.iter())
                .all(|(a, b)| a.color == b.color);
            screen.stable_ticks = if confident >= 6 && same {
                screen.stable_ticks.saturating_add(1)
            } else {
                0
            };
            screen.live = live;
            screen.live_patches = Some(patches);
            if screen.stable_ticks >= 10 {
                capture(screen, now);
            }
        }
    }
    if screen.flash.is_some_and(|(until, _)| now >= until) {
        screen.flash = None;
    }

    // --- zero-copy preview texture ---
    if let (Some(rs), true) = (frame.wgpu_render_state(), cam_ready) {
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
                    source: wgpu::ExternalImageSource::HTMLVideoElement(video.clone()),
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
            if let Some(store) = &app.store {
                let _ = store.set_setting("cam_rot", &screen.rotation.to_string());
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
        screen.face_idx = 0;
        screen.stable_ticks = 0;
        screen.flash = None;
        screen.mini_base = FaceletCube::SOLVED;
        screen.mini_cube = FaceletCube::SOLVED;
        screen.mini_anim.clear();
    }
    // Force-capture when auto won't bite (tricky stickers).
    if small_button(ui, slot(0), "force", app.t(TextKey::Capture), true) && !in_flash {
        capture(screen, now);
    }
    if small_button(ui, slot(1), "manual", app.t(TextKey::EnterManually), false) {
        *next = Some(Screen::Solve(super::solve::SolveScreen::new_input(
            FaceletCube::SOLVED,
        )));
    }

    // --- all six captured: rebuild and hand off to review ---
    if screen.face_idx >= 6 && screen.flash.is_none() {
        let state = assemble(screen);
        *next = Some(Screen::Solve(super::solve::SolveScreen::new_input(state)));
    }
}

fn capture(screen: &mut ScanScreen, now: f64) {
    if screen.face_idx >= 6 {
        return;
    }
    if let Some(patches) = screen.live_patches {
        // Flash what was detected in the grid for a moment.
        let classes = classify_face(&patches, &screen.cal).map(|c| c.color);
        screen.flash = Some((now + 0.9, classes));
        screen.captured[screen.face_idx] = Some(patches);
        // Bake the rotation the user just performed into the mini guide.
        if let Ok(alg) = Alg::parse(MINI_ALGS[screen.face_idx]) {
            screen.mini_base.apply_alg(&alg);
        }
        screen.mini_cube = screen.mini_base;
        screen.mini_anim.clear();
        screen.face_idx += 1;
        screen.stable_ticks = 0;
    }
}

/// Rebuild the facelet cube: recalibrate on the six centers, classify all
/// stickers against them, map classes to faces via capture order.
fn assemble(screen: &ScanScreen) -> FaceletCube {
    let centers: [Oklab; 6] = core::array::from_fn(|k| screen.captured[k].expect("captured")[4]);
    // Center-based recalibration assumes six clean colored centers. On
    // cubes with dark/logo centers that would poison every reference —
    // fall back to the default sticker palette (face identity comes from
    // the capture ORDER either way, never from the center color).
    let default_cal = Calibration::default_stickers();
    let centers_usable = centers
        .iter()
        .all(|&c| cube_vision::classify_one(c, &default_cal).color.is_some());
    let cal = if centers_usable {
        Calibration::from_centers(centers)
    } else {
        default_cal
    };
    let mut cube = FaceletCube::SOLVED;
    for (k, &face) in ORDER.iter().enumerate() {
        let patches = screen.captured[k].expect("captured");
        for (grid_pos, &facelet_off) in CAPTURE_GRID_TO_FACELET[k].iter().enumerate() {
            let class = cube_vision::classify_one(patches[grid_pos], &cal)
                .color
                .unwrap_or(k as u8);
            cube.0[face as usize * 9 + facelet_off as usize] = ORDER[class as usize];
        }
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
    let grid_stroke = Stroke::new(2.0, Color32::from_white_alpha(190));
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
    let t = f32::from(screen.stable_ticks.min(10)) / 10.0;
    let bar_bg = Rect::from_min_size(
        Pos2::new(square.left(), square.top() - 16.0),
        Vec2::new(side, 6.0),
    );
    p.rect_filled(bar_bg, 3.0, Color32::from_black_alpha(140));
    if t > 0.0 && screen.face_idx < 6 {
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
