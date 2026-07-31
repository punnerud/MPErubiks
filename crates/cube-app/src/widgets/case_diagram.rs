//! Standard OLL/PLL-style case diagrams: the U face from above with the
//! 12 last-layer side stickers as thin bars around it. Drawn straight from
//! the case's canonical state, so diagrams can never drift from the data.

use cube_core::{Face, FaceletCube};
use egui::{Color32, Pos2, Rect, Ui, Vec2};

use super::net2d::face_color32;

/// Draw the last-layer diagram of `state` into `rect`.
/// For OLL-style diagrams pass `orientation_only = true`: U-color stickers
/// draw yellow-ish (the U color), everything else neutral gray.
pub fn draw_ll_diagram(ui: &Ui, rect: Rect, state: &FaceletCube, orientation_only: bool) {
    let p = ui.painter();
    let bar = rect.width() * 0.10;
    let inner = Rect::from_min_max(
        rect.min + Vec2::splat(bar + 2.0),
        rect.max - Vec2::splat(bar + 2.0),
    );
    let cell = inner.width() / 3.0;
    let u_color = state.center(Face::U);

    let paint = |color: Face| -> Color32 {
        if orientation_only && color != u_color {
            Color32::from_gray(70)
        } else {
            face_color32(color)
        }
    };

    // U face 3x3 (row-major U1..U9 matches looking from above, B at top).
    for row in 0..3 {
        for col in 0..3 {
            let idx = row * 3 + col;
            let r = Rect::from_min_size(
                inner.min + Vec2::new(col as f32 * cell, row as f32 * cell),
                Vec2::splat(cell),
            )
            .shrink(cell * 0.06);
            p.rect_filled(r, cell * 0.15, paint(state.0[idx]));
        }
    }

    // Side bars: B row along the top edge (U1..U3 border B), F along the
    // bottom, L left, R right. Positions follow the Kociemba layout.
    // B1..B3 run right-to-left when viewed from above the B side.
    let side = |p0: Pos2, step: Vec2, size: Vec2, idxs: [usize; 3], reversed: bool| {
        for (k, &i) in idxs.iter().enumerate() {
            let k = if reversed { 2 - k } else { k };
            let r = Rect::from_min_size(p0 + step * k as f32, size).shrink(1.0);
            ui.painter().rect_filled(r, 2.0, paint(state.0[i]));
        }
    };
    let hsize = Vec2::new(cell, bar);
    let vsize = Vec2::new(bar, cell);
    // Top edge: B face top row (B1 B2 B3), B1 sits top-right from above.
    side(
        Pos2::new(inner.left(), rect.top()),
        Vec2::new(cell, 0.0),
        hsize,
        [45, 46, 47],
        true,
    );
    // Bottom edge: F top row (F1 F2 F3), F1 bottom-left.
    side(
        Pos2::new(inner.left(), inner.bottom() + 2.0),
        Vec2::new(cell, 0.0),
        hsize,
        [18, 19, 20],
        false,
    );
    // Left edge: L top row (L1 L2 L3), L1 top-left (borders B).
    side(
        Pos2::new(rect.left(), inner.top()),
        Vec2::new(0.0, cell),
        vsize,
        [36, 37, 38],
        false,
    );
    // Right edge: R top row (R1 R2 R3), R1 borders F -> bottom-right.
    side(
        Pos2::new(inner.right() + 2.0, inner.top()),
        Vec2::new(0.0, cell),
        vsize,
        [9, 10, 11],
        true,
    );
}
