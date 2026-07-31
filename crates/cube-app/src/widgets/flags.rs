//! Painter-drawn language flags (egui's default fonts have no flag emoji,
//! and two 24x16 icons don't justify an image dependency).

use crate::i18n::Lang;
use egui::{Color32, Pos2, Rect, Sense, Stroke, Ui, Vec2};

/// A clickable flag; draws a selection ring when active. Returns true when
/// clicked.
pub fn flag_button(ui: &mut Ui, lang: Lang, active: bool) -> bool {
    let size = Vec2::new(34.0, 24.0);
    let (rect, response) = ui.allocate_exact_size(size + Vec2::splat(6.0), Sense::click());
    let flag_rect = Rect::from_center_size(rect.center(), size);
    let p = ui.painter();
    match lang {
        Lang::No => draw_norwegian(p, flag_rect),
        Lang::En => draw_union_jack(p, flag_rect),
    }
    let ring = if active {
        Stroke::new(2.5, ui.visuals().selection.stroke.color)
    } else if response.hovered() {
        Stroke::new(1.5, Color32::GRAY)
    } else {
        Stroke::new(1.0, Color32::from_gray(90))
    };
    p.rect_stroke(flag_rect, 3.0, ring, egui::StrokeKind::Outside);
    response.clicked()
}

fn draw_norwegian(p: &egui::Painter, r: Rect) {
    p.rect_filled(r, 2.0, Color32::from_rgb(0xBA, 0x0C, 0x2F));
    // White cross underlay, blue cross overlay; vertical bar offset left.
    let cx = r.left() + r.width() * 0.36;
    let cy = r.center().y;
    let white = Color32::WHITE;
    let blue = Color32::from_rgb(0x00, 0x20, 0x5B);
    p.rect_filled(
        Rect::from_min_max(Pos2::new(cx - 5.0, r.top()), Pos2::new(cx + 5.0, r.bottom())),
        0.0,
        white,
    );
    p.rect_filled(
        Rect::from_min_max(Pos2::new(r.left(), cy - 5.0), Pos2::new(r.right(), cy + 5.0)),
        0.0,
        white,
    );
    p.rect_filled(
        Rect::from_min_max(Pos2::new(cx - 2.5, r.top()), Pos2::new(cx + 2.5, r.bottom())),
        0.0,
        blue,
    );
    p.rect_filled(
        Rect::from_min_max(Pos2::new(r.left(), cy - 2.5), Pos2::new(r.right(), cy + 2.5)),
        0.0,
        blue,
    );
}

fn draw_union_jack(p: &egui::Painter, r: Rect) {
    let blue = Color32::from_rgb(0x01, 0x21, 0x69);
    let red = Color32::from_rgb(0xC8, 0x10, 0x2E);
    p.rect_filled(r, 2.0, blue);
    // Simplified: white diagonals, then red diagonals, then the main cross.
    for (a, b) in [
        (r.left_top(), r.right_bottom()),
        (r.left_bottom(), r.right_top()),
    ] {
        p.line_segment([a, b], Stroke::new(5.0, Color32::WHITE));
        p.line_segment([a, b], Stroke::new(2.0, red));
    }
    let c = r.center();
    p.rect_filled(
        Rect::from_min_max(Pos2::new(c.x - 4.5, r.top()), Pos2::new(c.x + 4.5, r.bottom())),
        0.0,
        Color32::WHITE,
    );
    p.rect_filled(
        Rect::from_min_max(Pos2::new(r.left(), c.y - 4.5), Pos2::new(r.right(), c.y + 4.5)),
        0.0,
        Color32::WHITE,
    );
    p.rect_filled(
        Rect::from_min_max(Pos2::new(c.x - 2.5, r.top()), Pos2::new(c.x + 2.5, r.bottom())),
        0.0,
        red,
    );
    p.rect_filled(
        Rect::from_min_max(Pos2::new(r.left(), c.y - 2.5), Pos2::new(r.right(), c.y + 2.5)),
        0.0,
        red,
    );
}
