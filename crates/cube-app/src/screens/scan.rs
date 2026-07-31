//! Camera scan flow (browser only): live preview with a 3x3 grid overlay
//! and per-sticker color indicators, one face captured at a time in the
//! order F R B L U D, with pictogram instructions for how to turn the cube
//! between captures. After six faces the state is rebuilt (colors assigned
//! from centers, so kids can hold the cube any way they like) and handed to
//! the solve flow's review net.

use crate::app::{AsyncMsg, RubiksApp, Screen};
use crate::i18n::TextKey;
use crate::platform::web::camera::Camera;
use crate::widgets::icons;
use cube_core::{Face, FaceletCube};
use cube_vision::{classify_face, Calibration, Classified, Oklab};
use egui::{Color32, Pos2, Rect, RichText, Sense, Stroke, StrokeKind, Ui, Vec2};

/// Capture order and the face each capture becomes.
const ORDER: [Face; 6] = [Face::F, Face::R, Face::B, Face::L, Face::U, Face::D];

/// Camera-grid position (row-major) -> facelet offset within the face.
/// With the prescribed rotation sequence every face maps identity; kept as
/// data so a physical-cube discrepancy is a table fix, not a code hunt.
const CAPTURE_GRID_TO_FACELET: [[u8; 9]; 6] = [[0, 1, 2, 3, 4, 5, 6, 7, 8]; 6];

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
    /// Sampled sticker colors (Oklab) per captured face, capture order.
    captured: [Option<[Oklab; 9]>; 6],
    face_idx: usize,
    live: [Classified; 9],
    live_patches: Option<[Oklab; 9]>,
    stable_ticks: u8,
    last_sample: f64,
    cal: Calibration,
}

impl ScanScreen {
    pub fn new(app: &RubiksApp, ctx: &egui::Context) -> Self {
        let tx = app.tx.clone();
        let ctx = ctx.clone();
        wasm_bindgen_futures::spawn_local(async move {
            let result = Camera::open().await;
            let _ = tx.send(AsyncMsg::CameraReady(result.map_err(|e| e)));
            ctx.request_repaint();
        });
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
                // Fall back to manual entry.
                if big_button(ui, app.t(TextKey::EnterManually), Color32::from_rgb(0x2A, 0x5C, 0xC2)) {
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
    // Scoped camera reads: the mutations below must not overlap the borrow.
    let (cam_ready, dims, video) = {
        let CameraState::Ready(cam) = &screen.camera else {
            return;
        };
        (cam.ready(), cam.dims(), cam.video().clone())
    };
    let now = ui.input(|i| i.time);
    ui.ctx().request_repaint(); // live preview

    // --- classification tick at 10 Hz ---
    if cam_ready && now - screen.last_sample > 0.1 {
        screen.last_sample = now;
        let patches = match &screen.camera {
            CameraState::Ready(cam) => cam.sample_patches(),
            _ => None,
        };
        if let Some(patches) = patches {
            let live = classify_face(&patches, &screen.cal);
            let all_confident = live
                .iter()
                .all(|c| c.color.is_some() && c.confidence > 0.5);
            let same_as_before = screen
                .live
                .iter()
                .zip(live.iter())
                .all(|(a, b)| a.color == b.color);
            screen.stable_ticks = if all_confident && same_as_before {
                screen.stable_ticks.saturating_add(1)
            } else {
                0
            };
            screen.live = live;
            screen.live_patches = Some(patches);
            // Auto-capture after ~1s of stability.
            if screen.stable_ticks >= 10 {
                capture(screen);
            }
        }
    }

    // --- zero-copy preview texture ---
    if let (Some(rs), true) = (frame.wgpu_render_state(), cam_ready) {
        let recreate = screen
            .preview
            .as_ref()
            .map_or(true, |p| p.size != dims);
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

    // --- layout: preview (aspect-fit) + overlay, then controls ---
    let controls_height = 150.0;
    let avail = Vec2::new(
        ui.available_width(),
        (ui.available_height() - controls_height).max(160.0),
    );
    let (outer, _) = ui.allocate_exact_size(avail, Sense::hover());

    if let Some(preview) = &screen.preview {
        let (vw, vh) = (preview.size.0 as f32, preview.size.1 as f32);
        let scale = (outer.width() / vw).min(outer.height() / vh);
        let shown = Vec2::new(vw * scale, vh * scale);
        let rect = Rect::from_center_size(outer.center(), shown);
        ui.painter().image(
            preview.id,
            rect,
            Rect::from_min_max(Pos2::ZERO, Pos2::new(1.0, 1.0)),
            Color32::WHITE,
        );
        draw_overlay(ui, rect, screen);
    }

    // --- bottom controls ---
    ui.vertical_centered(|ui| {
        face_progress_row(ui, screen);
        instruction_row(app, ui, screen);
        ui.horizontal(|ui| {
            ui.add_space((ui.available_width() - 240.0).max(0.0) / 2.0);
            // Manual capture (always available).
            if icons::big_icon_button(
                ui,
                Vec2::new(110.0, 58.0),
                Color32::from_rgb(0x1E, 0x88, 0x50),
                app.t(TextKey::Capture),
                icons::draw_camera,
            )
            .clicked()
            {
                capture(screen);
            }
            // Skip to manual entry.
            if icons::big_icon_button(
                ui,
                Vec2::new(110.0, 58.0),
                Color32::from_gray(60),
                app.t(TextKey::EnterManually),
                |p, r| icons::draw_mini_cube(p, r.shrink(6.0), icons::scrambled_face()),
            )
            .clicked()
            {
                *next = Some(Screen::Solve(super::solve::SolveScreen::new_input(
                    FaceletCube::SOLVED,
                )));
            }
        });
    });

    // --- all six captured: rebuild the cube and hand off to review ---
    if screen.face_idx >= 6 {
        let state = assemble(screen);
        *next = Some(Screen::Solve(super::solve::SolveScreen::new_input(state)));
    }
}

fn capture(screen: &mut ScanScreen) {
    if screen.face_idx >= 6 {
        return;
    }
    if let Some(patches) = screen.live_patches {
        screen.captured[screen.face_idx] = Some(patches);
        screen.face_idx += 1;
        screen.stable_ticks = 0;
    }
}

/// Rebuild the facelet cube from the six captured faces: recalibrate on
/// the six centers, classify every sticker against them, and map classes
/// to faces via the capture order.
fn assemble(screen: &ScanScreen) -> FaceletCube {
    let centers: [Oklab; 6] = core::array::from_fn(|k| {
        screen.captured[k].expect("all captured")[4]
    });
    let cal = Calibration::from_centers(centers);
    let mut cube = FaceletCube::SOLVED;
    for (k, &face) in ORDER.iter().enumerate() {
        let patches = screen.captured[k].expect("all captured");
        for (grid_pos, &facelet_off) in CAPTURE_GRID_TO_FACELET[k].iter().enumerate() {
            let class = cube_vision::classify_one(patches[grid_pos], &cal)
                .color
                .unwrap_or(k as u8); // hopeless sticker: guess this face
            cube.0[face as usize * 9 + facelet_off as usize] = ORDER[class as usize];
        }
    }
    cube
}

fn draw_overlay(ui: &Ui, rect: Rect, screen: &ScanScreen) {
    let p = ui.painter();
    let side = rect.width().min(rect.height()) * 0.6;
    let square = Rect::from_center_size(rect.center(), Vec2::splat(side));

    // Dim everything outside the guide square.
    for r in [
        Rect::from_min_max(rect.min, Pos2::new(rect.max.x, square.min.y)),
        Rect::from_min_max(Pos2::new(rect.min.x, square.max.y), rect.max),
        Rect::from_min_max(Pos2::new(rect.min.x, square.min.y), Pos2::new(square.min.x, square.max.y)),
        Rect::from_min_max(Pos2::new(square.max.x, square.min.y), Pos2::new(rect.max.x, square.max.y)),
    ] {
        p.rect_filled(r, 0.0, Color32::from_black_alpha(110));
    }

    let cell = side / 3.0;
    let grid_stroke = Stroke::new(2.0, Color32::from_white_alpha(180));
    for i in 0..=3 {
        let x = square.left() + i as f32 * cell;
        p.line_segment([Pos2::new(x, square.top()), Pos2::new(x, square.bottom())], grid_stroke);
        let y = square.top() + i as f32 * cell;
        p.line_segment([Pos2::new(square.left(), y), Pos2::new(square.right(), y)], grid_stroke);
    }

    // Per-cell classification dot + confidence ring.
    for row in 0..3 {
        for col in 0..3 {
            let c = &screen.live[row * 3 + col];
            let center = Pos2::new(
                square.left() + (col as f32 + 0.5) * cell,
                square.top() + (row as f32 + 0.5) * cell,
            );
            let color = c
                .color
                .map(|cls| class_color(cls))
                .unwrap_or(Color32::from_gray(120));
            p.circle_filled(center, cell * 0.14, color);
            p.circle_stroke(
                center,
                cell * 0.2,
                Stroke::new(
                    3.0,
                    if c.confidence > 0.5 && c.color.is_some() {
                        Color32::from_rgb(0x4C, 0xD9, 0x64)
                    } else {
                        Color32::from_white_alpha(60)
                    },
                ),
            );
        }
    }

    // Stability ring: fills as the hold-steady counter climbs.
    let t = f32::from(screen.stable_ticks.min(10)) / 10.0;
    if t > 0.0 && screen.face_idx < 6 {
        let radius = side * 0.52;
        let n = (t * 40.0) as usize;
        let pts: Vec<Pos2> = (0..=n)
            .map(|i| {
                let a = -std::f32::consts::FRAC_PI_2
                    + std::f32::consts::TAU * (i as f32 / 40.0);
                Pos2::new(
                    rect.center().x + radius * a.cos(),
                    rect.center().y + radius * a.sin(),
                )
            })
            .collect();
        for w in pts.windows(2) {
            p.line_segment([w[0], w[1]], Stroke::new(5.0, Color32::from_rgb(0x4C, 0xD9, 0x64)));
        }
    }
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

/// Six mini-squares showing which faces are captured (green check) and
/// which is current (pulsing outline).
fn face_progress_row(ui: &mut Ui, screen: &ScanScreen) {
    ui.horizontal(|ui| {
        ui.add_space((ui.available_width() - 6.0 * 34.0).max(0.0) / 2.0);
        for k in 0..6 {
            let (rect, _) = ui.allocate_exact_size(Vec2::splat(28.0), Sense::hover());
            let p = ui.painter();
            let done = screen.captured[k].is_some();
            p.rect_filled(
                rect,
                6.0,
                if done {
                    Color32::from_rgb(0x1E, 0x88, 0x50)
                } else {
                    Color32::from_gray(55)
                },
            );
            if done {
                icons::draw_check(p, rect.shrink(7.0));
            }
            if k == screen.face_idx {
                p.rect_stroke(rect, 6.0, Stroke::new(2.5, Color32::WHITE), StrokeKind::Outside);
            }
        }
    });
}

/// Pictogram instruction for how to move the cube before this capture.
fn instruction_row(app: &RubiksApp, ui: &mut Ui, screen: &ScanScreen) {
    let key = match screen.face_idx {
        0 => TextKey::ScanHoldSteady,
        1..=3 => TextKey::ScanTurnLeft,
        4 => TextKey::ScanTiltUp,
        _ => TextKey::ScanTiltDown,
    };
    ui.horizontal(|ui| {
        ui.add_space((ui.available_width() - 260.0).max(0.0) / 2.0);
        let (rect, _) = ui.allocate_exact_size(Vec2::splat(30.0), Sense::hover());
        match screen.face_idx {
            1..=3 => icons::draw_turn_left(ui.painter(), rect),
            4 => icons::draw_tilt_up(ui.painter(), rect),
            5 => icons::draw_tilt_down(ui.painter(), rect),
            _ => icons::draw_camera(ui.painter(), rect),
        }
        ui.label(RichText::new(app.t(key)).size(18.0));
    });
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
