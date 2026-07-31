//! Shared playback widgets: karaoke move letters and speed chevrons —
//! used by both the lessons and the solve guide.

use crate::app::RubiksApp;
use crate::widgets::icons;
use cube_core::Move;
use egui::{Color32, RichText, Ui, Vec2};

/// The sequence's letters, karaoke-style: played moves dim green, the move
/// being animated RIGHT NOW is big and bright, upcoming ones gray.
pub fn karaoke_row(ui: &mut Ui, moves: &[Move], played: usize, animating: bool) {
    if moves.is_empty() {
        return;
    }
    ui.horizontal_wrapped(|ui| {
        ui.add_space((ui.available_width() - moves.len() as f32 * 34.0).max(0.0) / 2.0);
        for (i, m) in moves.iter().enumerate() {
            let text = m.to_string();
            let rich = if i + 1 == played && animating {
                RichText::new(text).size(30.0).strong().color(Color32::WHITE)
            } else if i < played {
                RichText::new(text)
                    .size(20.0)
                    .color(Color32::from_rgb(0x4C, 0xD9, 0x64))
            } else {
                RichText::new(text).size(20.0).color(Color32::from_gray(110))
            };
            ui.label(rich);
        }
    });
}

/// Slower / faster chevron buttons for animation playback.
pub fn speed_buttons(app: &mut RubiksApp, ui: &mut Ui) {
    speed_buttons_sized(app, ui, Vec2::new(64.0, 64.0))
}

pub fn speed_buttons_sized(app: &mut RubiksApp, ui: &mut Ui, size: Vec2) {
    let gray = Color32::from_gray(60);
    if icons::big_icon_button(ui, size, gray, "", icons::draw_chevrons_left).clicked() {
        app.animator.secs_per_quarter = (app.animator.secs_per_quarter * 1.5).min(0.7);
    }
    if icons::big_icon_button(ui, size, gray, "", icons::draw_chevrons_right).clicked() {
        app.animator.secs_per_quarter = (app.animator.secs_per_quarter / 1.5).max(0.08);
    }
}
