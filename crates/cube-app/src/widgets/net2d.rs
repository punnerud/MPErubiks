//! 2D unrolled cube net editor: the child-friendly way to enter or fix a
//! cube state — tap a sticker to cycle its color (right-click/long-press
//! cycles backwards). Centers are locked (they define the color scheme).

use cube_core::{Face, FaceletCube};
use egui::{Color32, Pos2, Rect, Sense, Stroke, StrokeKind, Ui, Vec2};

pub fn face_color32(c: Face) -> Color32 {
    match c {
        Face::U => Color32::from_rgb(0xF5, 0xF5, 0xF5),
        Face::R => Color32::from_rgb(0xE0, 0x1B, 0x2E),
        Face::F => Color32::from_rgb(0x00, 0xA8, 0x60),
        Face::D => Color32::from_rgb(0xFF, 0xD5, 0x00),
        Face::L => Color32::from_rgb(0xFF, 0x61, 0x00),
        Face::B => Color32::from_rgb(0x0D, 0x5C, 0xC7),
    }
}

fn next_color(c: Face, backwards: bool) -> Face {
    let i = c as usize;
    let j = if backwards { (i + 5) % 6 } else { (i + 1) % 6 };
    Face::from_index(j)
}

/// Draws the unrolled net (U on top; L F R B in the middle row; D below).
/// Returns true if the state was edited. `editable` gates interaction so
/// the same widget can display read-only scan results.
pub fn net_editor(ui: &mut Ui, state: &mut FaceletCube, editable: bool) -> bool {
    let cell = (ui.available_width() / 13.0).clamp(18.0, 40.0);
    let face_size = cell * 3.0;
    let gap = cell * 0.15;
    let total = Vec2::new(
        face_size * 4.0 + gap * 3.0,
        face_size * 3.0 + gap * 2.0,
    );
    let (rect, _) = ui.allocate_exact_size(total, Sense::hover());
    let mut changed = false;

    // Face origin positions in the net, in face-size units (col, row).
    let placements: [(Face, f32, f32); 6] = [
        (Face::U, 1.0, 0.0),
        (Face::L, 0.0, 1.0),
        (Face::F, 1.0, 1.0),
        (Face::R, 2.0, 1.0),
        (Face::B, 3.0, 1.0),
        (Face::D, 1.0, 2.0),
    ];

    for (face, fc, fr) in placements {
        let origin = Pos2::new(
            rect.left() + fc * (face_size + gap),
            rect.top() + fr * (face_size + gap),
        );
        for row in 0..3 {
            for col in 0..3 {
                let idx = face as usize * 9 + row * 3 + col;
                let cr = Rect::from_min_size(
                    origin + Vec2::new(col as f32 * cell, row as f32 * cell),
                    Vec2::splat(cell),
                )
                .shrink(cell * 0.05);
                let is_center = row == 1 && col == 1;
                let id = ui.id().with(("net", idx));
                let response = ui.interact(cr, id, Sense::click());
                let p = ui.painter();
                p.rect_filled(cr, cell * 0.18, face_color32(state.0[idx]));
                if is_center {
                    // Lock marker: small dark dot.
                    p.circle_filled(cr.center(), cell * 0.08, Color32::from_black_alpha(120));
                }
                if editable && !is_center {
                    if response.hovered() {
                        p.rect_stroke(
                            cr,
                            cell * 0.18,
                            Stroke::new(2.0, Color32::WHITE),
                            StrokeKind::Outside,
                        );
                    }
                    if response.clicked() {
                        state.0[idx] = next_color(state.0[idx], false);
                        changed = true;
                    }
                    if response.secondary_clicked() {
                        state.0[idx] = next_color(state.0[idx], true);
                        changed = true;
                    }
                }
            }
        }
    }
    changed
}
