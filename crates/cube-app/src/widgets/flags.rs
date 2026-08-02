//! Painter-drawn language flags. 31 bitmap flags would dwarf the app and
//! egui's fonts have no flag emoji, so each flag is a tiny declarative
//! spec (see [`crate::i18n::Flag`]) rendered with a handful of shapes.
//! They are recognizable at 34x24, which is all a picker row needs.

use crate::i18n::{Flag, Lang};
use egui::{Color32, Pos2, Rect, Sense, Stroke, Ui, Vec2};

fn rgb(c: u32) -> Color32 {
    Color32::from_rgb((c >> 16) as u8, ((c >> 8) & 0xFF) as u8, (c & 0xFF) as u8)
}

/// A clickable flag; draws a selection ring when active. Returns true
/// when clicked.
pub fn flag_button(ui: &mut Ui, lang: Lang, active: bool) -> bool {
    let size = Vec2::new(34.0, 24.0);
    let (rect, response) = ui.allocate_exact_size(size + Vec2::splat(6.0), Sense::click());
    let flag_rect = Rect::from_center_size(rect.center(), size);
    draw_flag(ui.painter(), flag_rect, lang.def().flag);
    let ring = if active {
        Stroke::new(2.5, ui.visuals().selection.stroke.color)
    } else if response.hovered() {
        Stroke::new(1.5, Color32::GRAY)
    } else {
        Stroke::new(1.0, Color32::from_gray(90))
    };
    ui.painter()
        .rect_stroke(flag_rect, 3.0, ring, egui::StrokeKind::Outside);
    response.clicked()
}

pub fn draw_flag(p: &egui::Painter, r: Rect, flag: Flag) {
    match flag {
        Flag::HBands(colors) => {
            let h = r.height() / colors.len() as f32;
            for (i, &c) in colors.iter().enumerate() {
                let top = r.top() + i as f32 * h;
                p.rect_filled(
                    Rect::from_min_max(Pos2::new(r.left(), top), Pos2::new(r.right(), top + h)),
                    0.0,
                    rgb(c),
                );
            }
        }
        Flag::VBands(colors) => {
            let w = r.width() / colors.len() as f32;
            for (i, &c) in colors.iter().enumerate() {
                let left = r.left() + i as f32 * w;
                p.rect_filled(
                    Rect::from_min_max(Pos2::new(left, r.top()), Pos2::new(left + w, r.bottom())),
                    0.0,
                    rgb(c),
                );
            }
        }
        Flag::Nordic {
            field,
            cross,
            inner,
        } => {
            p.rect_filled(r, 0.0, rgb(field));
            // The cross sits left of center, Nordic style.
            let cx = r.left() + r.width() * 0.38;
            let cy = r.center().y;
            let arm = r.height() * 0.26;
            let bar = |p: &egui::Painter, w: f32, c: Color32| {
                p.rect_filled(
                    Rect::from_min_max(
                        Pos2::new(r.left(), cy - w / 2.0),
                        Pos2::new(r.right(), cy + w / 2.0),
                    ),
                    0.0,
                    c,
                );
                p.rect_filled(
                    Rect::from_min_max(
                        Pos2::new(cx - w / 2.0, r.top()),
                        Pos2::new(cx + w / 2.0, r.bottom()),
                    ),
                    0.0,
                    c,
                );
            };
            bar(p, arm, rgb(cross));
            if let Some(i) = inner {
                bar(p, arm * 0.45, rgb(i));
            }
        }
        Flag::Disc { field, disc } => {
            p.rect_filled(r, 0.0, rgb(field));
            p.circle_filled(r.center(), r.height() * 0.30, rgb(disc));
        }
        Flag::Star { field, star } => {
            p.rect_filled(r, 0.0, rgb(field));
            star_shape(
                p,
                Pos2::new(r.left() + r.width() * 0.28, r.center().y),
                r.height() * 0.28,
                rgb(star),
            );
        }
        Flag::Triangle { top, bottom, tri } => {
            let mid = r.center().y;
            p.rect_filled(
                Rect::from_min_max(r.left_top(), Pos2::new(r.right(), mid)),
                0.0,
                rgb(top),
            );
            p.rect_filled(
                Rect::from_min_max(Pos2::new(r.left(), mid), r.right_bottom()),
                0.0,
                rgb(bottom),
            );
            p.add(egui::Shape::convex_polygon(
                vec![
                    r.left_top(),
                    Pos2::new(r.left() + r.width() * 0.45, r.center().y),
                    r.left_bottom(),
                ],
                rgb(tri),
                Stroke::NONE,
            ));
        }
        Flag::Crescent { field, mark } => {
            p.rect_filled(r, 0.0, rgb(field));
            let c = Pos2::new(r.left() + r.width() * 0.36, r.center().y);
            let rad = r.height() * 0.27;
            p.circle_filled(c, rad, rgb(mark));
            p.circle_filled(c + Vec2::new(rad * 0.42, 0.0), rad * 0.82, rgb(field));
            star_shape(p, Pos2::new(c.x + rad * 1.55, c.y), rad * 0.5, rgb(mark));
        }
        Flag::Diamond {
            field,
            diamond,
            disc,
        } => {
            p.rect_filled(r, 0.0, rgb(field));
            let c = r.center();
            p.add(egui::Shape::convex_polygon(
                vec![
                    Pos2::new(c.x, r.top() + r.height() * 0.12),
                    Pos2::new(r.right() - r.width() * 0.10, c.y),
                    Pos2::new(c.x, r.bottom() - r.height() * 0.12),
                    Pos2::new(r.left() + r.width() * 0.10, c.y),
                ],
                rgb(diamond),
                Stroke::NONE,
            ));
            p.circle_filled(c, r.height() * 0.20, rgb(disc));
        }
        Flag::Canton { field, canton, sun } => {
            p.rect_filled(r, 0.0, rgb(field));
            let canton_rect =
                Rect::from_min_size(r.min, Vec2::new(r.width() * 0.5, r.height() * 0.5));
            p.rect_filled(canton_rect, 0.0, rgb(canton));
            let c = canton_rect.center();
            let rad = canton_rect.height() * 0.30;
            for i in 0..6 {
                let a = i as f32 * std::f32::consts::PI / 6.0;
                let d = Vec2::new(a.cos(), a.sin()) * rad * 1.5;
                p.line_segment([c - d, c + d], Stroke::new(rad * 0.35, rgb(sun)));
            }
            p.circle_filled(c, rad * 0.7, rgb(sun));
            p.circle_filled(c, rad * 0.45, rgb(canton));
        }
        Flag::Greek => {
            let blue = rgb(0x004C98);
            let white = Color32::WHITE;
            let h = r.height() / 9.0;
            for i in 0..9 {
                let top = r.top() + i as f32 * h;
                p.rect_filled(
                    Rect::from_min_max(Pos2::new(r.left(), top), Pos2::new(r.right(), top + h)),
                    0.0,
                    if i % 2 == 0 { blue } else { white },
                );
            }
            let canton = Rect::from_min_size(r.min, Vec2::splat(h * 5.0));
            p.rect_filled(canton, 0.0, blue);
            let c = canton.center();
            let w = h * 0.9;
            p.rect_filled(
                Rect::from_min_max(
                    Pos2::new(canton.left(), c.y - w / 2.0),
                    Pos2::new(canton.right(), c.y + w / 2.0),
                ),
                0.0,
                white,
            );
            p.rect_filled(
                Rect::from_min_max(
                    Pos2::new(c.x - w / 2.0, canton.top()),
                    Pos2::new(c.x + w / 2.0, canton.bottom()),
                ),
                0.0,
                white,
            );
        }
        Flag::Taegeuk => {
            p.rect_filled(r, 0.0, Color32::WHITE);
            let c = r.center();
            let rad = r.height() * 0.28;
            let red = rgb(0xCD2E3A);
            let blue = rgb(0x0047A0);
            // Red over blue with the two small circles that make the
            // taegeuk read at a glance.
            p.circle_filled(c, rad, red);
            p.add(egui::Shape::convex_polygon(
                vec![
                    Pos2::new(c.x - rad, c.y),
                    Pos2::new(c.x + rad, c.y),
                    Pos2::new(c.x + rad, c.y + rad),
                    Pos2::new(c.x - rad, c.y + rad),
                ],
                blue,
                Stroke::NONE,
            ));
            p.circle_filled(Pos2::new(c.x - rad / 2.0, c.y), rad / 2.0, red);
            p.circle_filled(Pos2::new(c.x + rad / 2.0, c.y), rad / 2.0, blue);
            // Four corner trigrams, abstracted to short black bars.
            for (dx, dy) in [
                (-0.72f32, -0.62f32),
                (0.72, -0.62),
                (-0.72, 0.62),
                (0.72, 0.62),
            ] {
                let pos = Pos2::new(c.x + dx * r.width() * 0.5, c.y + dy * r.height() * 0.5);
                for k in -1..=1 {
                    p.line_segment(
                        [
                            Pos2::new(pos.x - r.width() * 0.08, pos.y + k as f32 * 3.0),
                            Pos2::new(pos.x + r.width() * 0.08, pos.y + k as f32 * 3.0),
                        ],
                        Stroke::new(1.2, Color32::BLACK),
                    );
                }
            }
        }
        Flag::UnionJack => draw_union_jack(p, r),
    }
}

fn star_shape(p: &egui::Painter, c: Pos2, radius: f32, color: Color32) {
    let mut pts = Vec::with_capacity(10);
    for i in 0..10 {
        let rad = if i % 2 == 0 { radius } else { radius * 0.45 };
        let a = -std::f32::consts::FRAC_PI_2 + i as f32 * std::f32::consts::PI / 5.0;
        pts.push(Pos2::new(c.x + rad * a.cos(), c.y + rad * a.sin()));
    }
    p.add(egui::Shape::convex_polygon(pts, color, Stroke::NONE));
}

fn draw_union_jack(p: &egui::Painter, r: Rect) {
    let blue = Color32::from_rgb(0x01, 0x22, 0x69);
    let red = Color32::from_rgb(0xC8, 0x10, 0x2E);
    p.rect_filled(r, 2.0, blue);
    let (w, h) = (r.width(), r.height());
    for (stroke_w, color) in [(5.0, Color32::WHITE), (2.0, red)] {
        p.line_segment(
            [r.left_top(), r.right_bottom()],
            Stroke::new(stroke_w, color),
        );
        p.line_segment(
            [r.right_top(), r.left_bottom()],
            Stroke::new(stroke_w, color),
        );
    }
    for (bar, color) in [(0.30, Color32::WHITE), (0.18, red)] {
        p.rect_filled(
            Rect::from_center_size(r.center(), Vec2::new(w, h * bar)),
            0.0,
            color,
        );
        p.rect_filled(
            Rect::from_center_size(r.center(), Vec2::new(w * bar * 0.7, h)),
            0.0,
            color,
        );
    }
}
