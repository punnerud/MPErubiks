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
/// Settings gear: six CLEAR square teeth around a ring with a hub hole
/// (the previous version's thick ring swallowed the teeth — it read as
/// a plain circle).
pub fn draw_gear(p: &egui::Painter, r: Rect) {
    let c = r.center();
    let radius = r.width().min(r.height()) * 0.26;
    let col = egui::Color32::from_gray(205);
    for i in 0..6 {
        let a = i as f32 * std::f32::consts::TAU / 6.0 + 0.26;
        let d = egui::Vec2::new(a.cos(), a.sin());
        let n = egui::Vec2::new(-d.y, d.x);
        let inner = c + d * (radius * 0.9);
        let outer = c + d * (radius * 1.65);
        let w = radius * 0.34;
        p.add(egui::Shape::convex_polygon(
            vec![inner + n * w, outer + n * w * 0.7, outer - n * w * 0.7, inner - n * w],
            col,
            egui::Stroke::NONE,
        ));
    }
    p.circle_filled(c, radius * 1.05, col);
    p.circle_filled(c, radius * 0.45, egui::Color32::from_gray(40));
}

/// A glowing light bulb: the universal "here's the answer" symbol.
pub fn draw_bulb(p: &egui::Painter, r: Rect) {
    let c = Pos2::new(r.center().x, r.top() + r.height() * 0.38);
    let rad = r.width().min(r.height()) * 0.30;
    let glass = egui::Color32::from_rgb(0xFF, 0xD5, 0x00);
    // Rays.
    for i in 0..7 {
        let a = -std::f32::consts::PI * 0.95 + i as f32 * std::f32::consts::PI * 0.9 / 6.0;
        let d = egui::Vec2::new(a.cos(), a.sin());
        p.line_segment(
            [c + d * (rad * 1.25), c + d * (rad * 1.7)],
            egui::Stroke::new(rad * 0.16, glass),
        );
    }
    // Glass + filament dot.
    p.circle_filled(c, rad, glass);
    p.circle_filled(c, rad * 0.4, egui::Color32::from_rgb(0xFF, 0xF3, 0xB0));
    // Base (screw cap).
    let base_w = rad * 0.9;
    let base_top = c.y + rad * 0.85;
    p.rect_filled(
        Rect::from_min_max(
            Pos2::new(c.x - base_w / 2.0, base_top),
            Pos2::new(c.x + base_w / 2.0, base_top + rad * 0.75),
        ),
        rad * 0.15,
        egui::Color32::from_gray(200),
    );
}

/// A small isometric SOLVED 3D cube: white top, green front-left,
/// red right — each visible face as a 3x3 sticker grid.
pub fn draw_iso_cube(p: &egui::Painter, r: Rect) {
    let c = r.center();
    let s = r.width().min(r.height()) * 0.46;
    let (w, h) = (s * 0.87, s * 0.5);
    let top = Pos2::new(c.x, c.y - s);
    let left = Pos2::new(c.x - w, c.y - h);
    let right = Pos2::new(c.x + w, c.y - h);
    let mid = c;
    let bl = Pos2::new(c.x - w, c.y + h);
    let br = Pos2::new(c.x + w, c.y + h);
    let bottom = Pos2::new(c.x, c.y + s);
    let lerp = |a: Pos2, b: Pos2, t: f32| Pos2::new(a.x + (b.x - a.x) * t, a.y + (b.y - a.y) * t);
    // Draw one face (quad q0..q3, row-major) as base + 3x3 stickers.
    let mut face = |q: [Pos2; 4], color: egui::Color32| {
        p.add(egui::Shape::convex_polygon(
            q.to_vec(),
            egui::Color32::from_gray(25),
            egui::Stroke::NONE,
        ));
        for i in 0..3 {
            for j in 0..3 {
                let (u0, u1) = (i as f32 / 3.0 + 0.02, (i as f32 + 1.0) / 3.0 - 0.02);
                let (v0, v1) = (j as f32 / 3.0 + 0.02, (j as f32 + 1.0) / 3.0 - 0.02);
                let at = |u: f32, v: f32| {
                    let a = lerp(q[0], q[1], u);
                    let b = lerp(q[3], q[2], u);
                    lerp(a, b, v)
                };
                p.add(egui::Shape::convex_polygon(
                    vec![at(u0, v0), at(u1, v0), at(u1, v1), at(u0, v1)],
                    color,
                    egui::Stroke::NONE,
                ));
            }
        }
    };
    // Top (white), front-left (green, slightly darker), right (red, darkest).
    face([top, right, mid, left], egui::Color32::from_rgb(0xF5, 0xF5, 0xF5));
    face([left, mid, bottom, bl], egui::Color32::from_rgb(0x00, 0x96, 0x56));
    face([mid, right, br, bottom], egui::Color32::from_rgb(0xC4, 0x1A, 0x2A));
}

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
pub fn draw_star(p: &egui::Painter, r: Rect) {
    let c = r.center();
    let outer = r.width().min(r.height()) / 2.2;
    let inner = outer * 0.45;
    let mut points = Vec::with_capacity(10);
    for i in 0..10 {
        let radius = if i % 2 == 0 { outer } else { inner };
        let a = -std::f32::consts::FRAC_PI_2 + i as f32 * std::f32::consts::PI / 5.0;
        points.push(egui::Pos2::new(c.x + radius * a.cos(), c.y + radius * a.sin()));
    }
    p.add(egui::Shape::convex_polygon(points, egui::Color32::WHITE, egui::Stroke::NONE));
}

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

/// Shuffle icon: two crossing arrows with prominent heads (a plain X reads
/// as "cancel" — the heads carry the meaning).
pub fn draw_shuffle(p: &egui::Painter, r: Rect) {
    let s = Stroke::new(r.height() * 0.11, Color32::WHITE);
    let (l, right) = (r.left(), r.right());
    let (t, b) = (r.top() + r.height() * 0.22, r.bottom() - r.height() * 0.22);
    p.line_segment([Pos2::new(l, t), Pos2::new(right - r.width() * 0.22, b)], s);
    p.line_segment([Pos2::new(l, b), Pos2::new(right - r.width() * 0.22, t)], s);
    for y in [t, b] {
        arrow_head(p, Pos2::new(right, y), r.width() * 0.3, s.color);
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

/// Curved arrow pointing left (turn the cube left).
pub fn draw_turn_left(p: &egui::Painter, r: Rect) {
    let c = r.center();
    let radius = r.width() * 0.36;
    let stroke = Stroke::new(r.height() * 0.12, Color32::WHITE);
    let n = 16;
    let mut prev: Option<Pos2> = None;
    for i in 0..=n {
        let a = -0.3 + 3.4 * (i as f32 / n as f32);
        let pt = Pos2::new(c.x + radius * a.cos(), c.y - radius * a.sin());
        if let Some(prev) = prev {
            p.line_segment([prev, pt], stroke);
        }
        prev = Some(pt);
    }
    if let Some(end) = prev {
        arrow_head_dir(p, end, Vec2::new(-r.width() * 0.2, r.height() * 0.12), stroke.color);
    }
}

/// Curved arrow pointing right (mirror of turn-left).
pub fn draw_turn_right(p: &egui::Painter, r: Rect) {
    let c = r.center();
    let radius = r.width() * 0.36;
    let stroke = Stroke::new(r.height() * 0.12, Color32::WHITE);
    let n = 16;
    let mut prev: Option<Pos2> = None;
    for i in 0..=n {
        let a = std::f32::consts::PI + 0.3 - 3.4 * (i as f32 / n as f32);
        let pt = Pos2::new(c.x + radius * a.cos(), c.y - radius * a.sin());
        if let Some(prev) = prev {
            p.line_segment([prev, pt], stroke);
        }
        prev = Some(pt);
    }
    if let Some(end) = prev {
        arrow_head_dir(p, end, Vec2::new(r.width() * 0.2, r.height() * 0.12), stroke.color);
    }
}

/// Arrow tilting up-toward-viewer.
pub fn draw_tilt_up(p: &egui::Painter, r: Rect) {
    let s = Stroke::new(r.height() * 0.12, Color32::WHITE);
    p.line_segment([Pos2::new(r.center().x, r.bottom()), Pos2::new(r.center().x, r.top())], s);
    arrow_head_dir(p, r.center_top(), Vec2::new(0.0, r.height() * 0.35), s.color);
}

/// Arrow tilting back down.
pub fn draw_tilt_down(p: &egui::Painter, r: Rect) {
    let s = Stroke::new(r.height() * 0.12, Color32::WHITE);
    p.line_segment([Pos2::new(r.center().x, r.top()), Pos2::new(r.center().x, r.bottom())], s);
    arrow_head_dir(p, r.center_bottom(), Vec2::new(0.0, -r.height() * 0.35), s.color);
}

fn arrow_head_dir(p: &egui::Painter, tip: Pos2, back: Vec2, color: Color32) {
    let ortho = Vec2::new(-back.y, back.x) * 0.5;
    p.add(egui::Shape::convex_polygon(
        vec![tip, tip + back + ortho, tip + back - ortho],
        color,
        Stroke::NONE,
    ));
}

/// Double chevrons: playback speed down / up.
pub fn draw_chevrons_left(p: &egui::Painter, r: Rect) {
    chevrons(p, r, -1.0)
}
pub fn draw_chevrons_right(p: &egui::Painter, r: Rect) {
    chevrons(p, r, 1.0)
}
fn chevrons(p: &egui::Painter, r: Rect, dir: f32) {
    let s = Stroke::new(r.height() * 0.14, Color32::WHITE);
    let h = r.height() * 0.42;
    for k in [-0.22f32, 0.22] {
        let x = r.center().x + k * r.width() - dir * r.width() * 0.12;
        let tip = Pos2::new(x + dir * r.width() * 0.24, r.center().y);
        p.line_segment([Pos2::new(x, r.center().y - h), tip], s);
        p.line_segment([Pos2::new(x, r.center().y + h), tip], s);
    }
}

/// Red-friendly cross (fail verdict).
pub fn draw_cross(p: &egui::Painter, r: Rect) {
    let s = Stroke::new(r.height() * 0.16, Color32::WHITE);
    let r = r.shrink(r.width() * 0.12);
    p.line_segment([r.left_top(), r.right_bottom()], s);
    p.line_segment([r.left_bottom(), r.right_top()], s);
}

/// Green-friendly checkmark.
pub fn draw_check(p: &egui::Painter, r: Rect) {
    let s = Stroke::new(r.height() * 0.16, Color32::WHITE);
    let a = Pos2::new(r.left() + r.width() * 0.1, r.center().y + r.height() * 0.05);
    let b = Pos2::new(r.left() + r.width() * 0.38, r.bottom() - r.height() * 0.1);
    let c = Pos2::new(r.right() - r.width() * 0.05, r.top() + r.height() * 0.12);
    p.line_segment([a, b], s);
    p.line_segment([b, c], s);
}

/// Play triangle.
pub fn draw_play(p: &egui::Painter, r: Rect) {
    p.add(egui::Shape::convex_polygon(
        vec![
            Pos2::new(r.left() + r.width() * 0.15, r.top()),
            Pos2::new(r.right(), r.center().y),
            Pos2::new(r.left() + r.width() * 0.15, r.bottom()),
        ],
        Color32::WHITE,
        Stroke::NONE,
    ));
}

/// Pause bars.
pub fn draw_pause(p: &egui::Painter, r: Rect) {
    for side in [-1.0f32, 1.0] {
        let bar = Rect::from_center_size(
            Pos2::new(r.center().x + side * r.width() * 0.2, r.center().y),
            Vec2::new(r.width() * 0.22, r.height() * 0.9),
        );
        p.rect_filled(bar, 3.0, Color32::WHITE);
    }
}

/// Forward arrow (next step).
/// Single-step back: a bar + one triangle pointing left.
pub fn draw_chevron_step_back(p: &egui::Painter, r: Rect) {
    let c = r.center();
    let h = r.height() * 0.28;
    let w = r.width() * 0.16;
    p.rect_filled(
        Rect::from_center_size(
            egui::Pos2::new(c.x - w * 1.4, c.y),
            egui::Vec2::new(w * 0.35, h * 2.0),
        ),
        1.0,
        egui::Color32::WHITE,
    );
    p.add(egui::Shape::convex_polygon(
        vec![
            egui::Pos2::new(c.x - w * 0.6, c.y),
            egui::Pos2::new(c.x + w * 1.2, c.y - h),
            egui::Pos2::new(c.x + w * 1.2, c.y + h),
        ],
        egui::Color32::WHITE,
        egui::Stroke::NONE,
    ));
}

/// Single-step forward: one triangle pointing right + a bar.
pub fn draw_chevron_step_fwd(p: &egui::Painter, r: Rect) {
    let c = r.center();
    let h = r.height() * 0.28;
    let w = r.width() * 0.16;
    p.add(egui::Shape::convex_polygon(
        vec![
            egui::Pos2::new(c.x + w * 0.6, c.y),
            egui::Pos2::new(c.x - w * 1.2, c.y - h),
            egui::Pos2::new(c.x - w * 1.2, c.y + h),
        ],
        egui::Color32::WHITE,
        egui::Stroke::NONE,
    ));
    p.rect_filled(
        Rect::from_center_size(
            egui::Pos2::new(c.x + w * 1.4, c.y),
            egui::Vec2::new(w * 0.35, h * 2.0),
        ),
        1.0,
        egui::Color32::WHITE,
    );
}

pub fn draw_next_arrow(p: &egui::Painter, r: Rect) {
    let s = Stroke::new(r.height() * 0.13, Color32::WHITE);
    let mid = r.center().y;
    p.line_segment([Pos2::new(r.left(), mid), Pos2::new(r.right(), mid)], s);
    p.line_segment(
        [Pos2::new(r.right(), mid), Pos2::new(r.right() - r.width() * 0.4, r.top())],
        s,
    );
    p.line_segment(
        [Pos2::new(r.right(), mid), Pos2::new(r.right() - r.width() * 0.4, r.bottom())],
        s,
    );
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
