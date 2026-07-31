//! Placeholder for screens still being built (Solve in M3/M4, Train in M5).

use crate::app::RubiksApp;
use crate::i18n::TextKey;
use crate::widgets::cube_view::CubeView;
use egui::{Ui, Vec2};

pub fn show(app: &mut RubiksApp, ui: &mut Ui) {
    super::play::top_bar(app, ui);
    ui.vertical_centered(|ui| {
        ui.heading(app.t(TextKey::ComingSoon));
    });
    let size = Vec2::new(ui.available_width(), ui.available_height().max(120.0));
    CubeView {
        cube: &app.cube,
        animator: &app.animator,
        orbit: &mut app.orbit,
        highlight: None,
        color_override: None,
    }
    .show(ui, size);
}
