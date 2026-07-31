//! Play mode: free 3D cube with scramble/reset/undo and tap-to-turn face
//! buttons. Buttons are colored like the faces themselves so pre-readers
//! can connect button -> face.

use crate::app::{RubiksApp, Screen};
use crate::i18n::TextKey;
use crate::widgets::{cube_view::CubeView, icons};
use cube_core::{FaceletCube, Move, Turns};
use egui::{Color32, Sense, Ui, Vec2};

pub fn show(app: &mut RubiksApp, ui: &mut Ui) {
    top_bar(app, ui);

    // Actions ABOVE the cube (thumb-reach + clear of the iOS bottom bar).
    ui.vertical_centered(|ui| {
        ui.horizontal(|ui| {
            ui.add_space((ui.available_width() - 3.0 * 92.0).max(0.0) / 2.0);
            action_buttons(app, ui);
        });
    });

    let controls_height = 96.0 + crate::app::BOTTOM_INSET;
    let cube_size = Vec2::new(
        ui.available_width(),
        (ui.available_height() - controls_height).max(120.0),
    );
    let highlight = app.selected_face.map(|f| Move::Face(f, Turns::Cw));
    let response = CubeView {
        cube: &app.cube,
        animator: &app.animator,
        orbit: &mut app.orbit,
        highlight,
        color_override: None,
    }
    .show(ui, cube_size);
    handle_tap_select(app, &response);

    ui.vertical_centered(|ui| {
        ui.horizontal(|ui| {
            ui.add_space((ui.available_width() - 2.0 * 108.0).max(0.0) / 2.0);
            turn_arrows(app, ui);
        });
        ui.add_space(crate::app::BOTTOM_INSET);
    });
}

/// Scramble / reset / undo — compact, above the cube.
fn action_buttons(app: &mut RubiksApp, ui: &mut Ui) {
    let action = Vec2::new(84.0, 64.0);
    if icons::big_icon_button(
        ui,
        action,
        Color32::from_rgb(0x8E, 0x36, 0xB8),
        app.t(TextKey::Scramble),
        icons::draw_shuffle,
    )
    .clicked()
    {
        app.scramble();
    }
    if icons::big_icon_button(
        ui,
        action,
        Color32::from_rgb(0x4A, 0x4F, 0x5C),
        app.t(TextKey::Reset),
        icons::draw_reset,
    )
    .clicked()
    {
        app.animator.clear();
        app.cube = FaceletCube::SOLVED;
        app.history.clear();
    }
    if icons::big_icon_button(
        ui,
        action,
        Color32::from_rgb(0x4A, 0x4F, 0x5C),
        app.t(TextKey::Undo),
        icons::draw_back_arrow,
    )
    .clicked()
    {
        if let Some(m) = app.history.pop() {
            app.animator.enqueue(m.inverse());
        }
    }
}

/// Tap a sticker: select (and pulse) its whole layer; tap again to
/// deselect. Shared by Play and the lessons sandbox.
pub fn handle_tap_select(app: &mut RubiksApp, response: &egui::Response) {
    if response.clicked() {
        if let Some(pos) = response.interact_pointer_pos() {
            let rect = response.rect;
            let ndc = (
                (pos.x - rect.left()) / rect.width() * 2.0 - 1.0,
                -((pos.y - rect.top()) / rect.height() * 2.0 - 1.0),
            );
            let hit = cube_render::pick_face(&app.orbit, rect.aspect_ratio(), ndc);
            app.selected_face = if hit == app.selected_face { None } else { hit };
        }
    }
}

/// Two big arrows that turn the tap-selected layer (animated). Colored
/// like the selected face so button and pulsing layer visibly belong
/// together.
pub fn turn_arrows(app: &mut RubiksApp, ui: &mut Ui) {
    let Some(face) = app.selected_face else {
        return;
    };
    let color = crate::widgets::net2d::face_color32(face);
    let size = Vec2::new(96.0, 76.0);
    for (turns, draw) in [
        (Turns::Ccw, icons::draw_turn_left as fn(&egui::Painter, egui::Rect)),
        (Turns::Cw, icons::draw_turn_right as fn(&egui::Painter, egui::Rect)),
    ] {
        if icons::big_icon_button(ui, size, color, &face.letter().to_string(), draw).clicked() {
            let m = Move::Face(face, turns);
            app.history.push(m);
            app.animator.enqueue(m);
        }
    }
}

pub fn top_bar(app: &mut RubiksApp, ui: &mut Ui) {
    egui::Panel::top("topbar").show_separator_line(false).show(ui, |ui| {
        ui.horizontal(|ui| {
            let (rect, response) =
                ui.allocate_exact_size(Vec2::new(56.0, 40.0), Sense::click());
            icons::draw_back_arrow(ui.painter(), rect.shrink2(Vec2::new(14.0, 10.0)));
            if response.clicked() {
                app.screen = Screen::Menu;
            }
        });
    });
}

