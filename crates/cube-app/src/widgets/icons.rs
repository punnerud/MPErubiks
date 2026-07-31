//! Painter-drawn icons for a child-friendly, reading-free UI. Every icon
//! is a small pure function of (painter, rect) so buttons stay visual.

use egui::{Color32, Pos2, Rect, Stroke, StrokeKind, Ui, Vec2};

/// A large tappable button with an icon and a small caption below it.
/// The caption supports pre-readers by being redundant, not required.
pub fn big_icon_button(
    ui: &mut Ui,
    size: Vec2,
    fill: Color32,
    caption: &str,
    draw: impl FnOnce(&egui::Painter, Rect),
) -> egui::Response {
    let (rect, response) = ui.allocate_exact_size(size, egui::Sense::click());
    let rounding = 18.0;
    let fill = if response.hovered() {
        fill.gamma_multiply(1.15)
    } else {
        fill
    };
    let p = ui.painter();
    p.rect_filled(rect, rounding, fill);
    if response.hovered() {
        p.rect_stroke(rect, rounding, Stroke::new(3.0, Color32::WHITE), StrokeKind::Inside);
    }
    let icon_rect = Rect::from_center_size(
        rect.center() - Vec2::new(0.0, size.y * 0.08),
        Vec2::splat(size.y * 0.45),
    );
    draw(p, icon_rect);
    p.text(
        Pos2::new(rect.center().x, rect.bottom() - size.y * 0.13),
        egui::Align2::CENTER_CENTER,
        caption,
        egui::FontId::proportional(size.y * 0.14),
        Color32::WHITE,
    );
    response
}

/// A 2D mini cube face (3x3 colored grid) — the app's "logo" and the
/// universal icon for cube-related actions.
pub fn draw_mini_cube(p: &egui::Painter, r: Rect, colors: [[Color32; 3]; 3]) {
    let cell = r.width() / 3.0;
    for (row, cols) in colors.iter().enumerate() {
        for (col, color) in cols.iter().enumerate() {
            let cr = Rect::from_min_size(
                Pos2::new(r.left() + col as f32 * cell, r.top() + row as f32 * cell),
                Vec2::splat(cell),
            )
            .shrink(cell * 0.06);
            p.rect_filled(cr, cell * 0.15, *color);
        }
    }
}

pub fn scrambled_face() -> [[Color32; 3]; 3] {
    let w = Color32::from_rgb(0xF5, 0xF5, 0xF5);
    let r = Color32::from_rgb(0xE0, 0x1B, 0x2E);
    let g = Color32::from_rgb(0x00, 0xA8, 0x60);
    let y = Color32::from_rgb(0xFF, 0xD5, 0x00);
    let o = Color32::from_rgb(0xFF, 0x61, 0x00);
    let b = Color32::from_rgb(0x0D, 0x5C, 0xC7);
    [[r, w, b], [g, y, o], [y, b, g]]
}

pub fn solved_face(color: Color32) -> [[Color32; 3]; 3] {
    [[color; 3]; 3]
}

/// Camera icon (for the scan/solve flow).
pub fn draw_camera(p: &egui::Painter, r: Rect) {
    let body = Rect::from_min_max(
        Pos2::new(r.left(), r.top() + r.height() * 0.22),
        r.right_bottom(),
    );
    p.rect_filled(body, r.width() * 0.12, Color32::WHITE);
    // Viewfinder bump.
    let bump = Rect::from_min_max(
        Pos2::new(r.center().x - r.width() * 0.18, r.top()),
        Pos2::new(r.center().x + r.width() * 0.18, r.top() + r.height() * 0.3),
    );
    p.rect_filled(bump, r.width() * 0.06, Color32::WHITE);
    p.circle_filled(body.center(), r.width() * 0.22, Color32::from_gray(40));
    p.circle_filled(body.center(), r.width() * 0.13, Color32::from_rgb(0x58, 0xB6, 0xFF));
}

/// Dumbbell icon (training).
pub fn draw_dumbbell(p: &egui::Painter, r: Rect) {
    let c = r.center();
    let bar = Rect::from_center_size(c, Vec2::new(r.width() * 0.9, r.height() * 0.14));
    p.rect_filled(bar, 4.0, Color32::WHITE);
    for side in [-1.0f32, 1.0] {
        for (dx, h) in [(0.32, 0.62), (0.44, 0.4)] {
            let w = Rect::from_center_size(
                Pos2::new(c.x + side * r.width() * dx, c.y),
                Vec2::new(r.width() * 0.1, r.height() * h),
            );
            p.rect_filled(w, 4.0, Color32::WHITE);
        }
    }
}

/// Shuffle icon: two crossing arrows.
pub fn draw_shuffle(p: &egui::Painter, r: Rect) {
    let s = Stroke::new(r.height() * 0.11, Color32::WHITE);
    let (l, right) = (r.left(), r.right());
    let (t, b) = (r.top() + r.height() * 0.2, r.bottom() - r.height() * 0.2);
    p.line_segment([Pos2::new(l, t), Pos2::new(right - r.width() * 0.15, b)], s);
    p.line_segment([Pos2::new(l, b), Pos2::new(right - r.width() * 0.15, t)], s);
    for y in [t, b] {
        arrow_head(p, Pos2::new(right, y), r.width() * 0.16, s.color);
    }
}

fn arrow_head(p: &egui::Painter, tip: Pos2, size: f32, color: Color32) {
    p.add(egui::Shape::convex_polygon(
        vec![
            tip,
            tip + Vec2::new(-size, -size * 0.55),
            tip + Vec2::new(-size, size * 0.55),
        ],
        color,
        Stroke::NONE,
    ));
}

/// Circular arrow (reset / turn direction).
pub fn draw_reset(p: &egui::Painter, r: Rect) {
    let c = r.center();
    let radius = r.width() * 0.38;
    let stroke = Stroke::new(r.height() * 0.11, Color32::WHITE);
    let n = 24;
    let mut prev: Option<Pos2> = None;
    for i in 0..=n {
        let a = 0.5 + 4.9 * (i as f32 / n as f32);
        let pt = Pos2::new(c.x + radius * a.cos(), c.y + radius * a.sin());
        if let Some(prev) = prev {
            p.line_segment([prev, pt], stroke);
        }
        prev = Some(pt);
    }
    let a_end = 0.5f32;
    let tip = Pos2::new(c.x + radius * a_end.cos(), c.y + radius * a_end.sin());
    arrow_head(p, tip + Vec2::new(0.0, -2.0), r.width() * 0.16, stroke.color);
}

/// Back arrow.
pub fn draw_back_arrow(p: &egui::Painter, r: Rect) {
    let s = Stroke::new(r.height() * 0.13, Color32::WHITE);
    let mid = r.center().y;
    p.line_segment([Pos2::new(r.left(), mid), Pos2::new(r.right(), mid)], s);
    p.line_segment(
        [Pos2::new(r.left(), mid), Pos2::new(r.left() + r.width() * 0.4, r.top())],
        s,
    );
    p.line_segment(
        [Pos2::new(r.left(), mid), Pos2::new(r.left() + r.width() * 0.4, r.bottom())],
        s,
    );
}
