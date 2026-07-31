//! Play mode: free 3D cube with scramble/reset/undo and tap-to-turn face
//! buttons. Buttons are colored like the faces themselves so pre-readers
//! can connect button -> face.

use crate::app::{RubiksApp, Screen};
use crate::i18n::TextKey;
use crate::widgets::{cube_view::CubeView, icons};
use cube_core::{Face, FaceletCube, Move, Turns};
use egui::{Color32, Rect, Sense, Ui, Vec2};

pub fn show(app: &mut RubiksApp, ui: &mut Ui) {
    top_bar(app, ui);

    // Bottom control panel first (fixed height), cube gets the rest.
    let controls_height = 130.0;
    let cube_size = Vec2::new(
        ui.available_width(),
        (ui.available_height() - controls_height).max(120.0),
    );
    CubeView {
        cube: &app.cube,
        animator: &app.animator,
        orbit: &mut app.orbit,
        highlight: None,
        color_override: None,
    }
    .show(ui, cube_size);

    ui.vertical_centered(|ui| {
        ui.horizontal_wrapped(|ui| {
            let action = Vec2::new(96.0, 76.0);
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
            ui.add_space(16.0);
            face_turn_buttons(app, ui);
        });
    });
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

/// Six face buttons colored like the sticker they turn, each with a small
/// rotation arrow. Tap = clockwise; the ⟲ toggle switches direction.
fn face_turn_buttons(app: &mut RubiksApp, ui: &mut Ui) {
    let colors = [
        (Face::U, Color32::from_rgb(0xF5, 0xF5, 0xF5)),
        (Face::R, Color32::from_rgb(0xE0, 0x1B, 0x2E)),
        (Face::F, Color32::from_rgb(0x00, 0xA8, 0x60)),
        (Face::D, Color32::from_rgb(0xFF, 0xD5, 0x00)),
        (Face::L, Color32::from_rgb(0xFF, 0x61, 0x00)),
        (Face::B, Color32::from_rgb(0x0D, 0x5C, 0xC7)),
    ];
    for (face, color) in colors {
        let (rect, response) = ui.allocate_exact_size(Vec2::splat(58.0), Sense::click());
        let p = ui.painter();
        p.rect_filled(rect, 12.0, color);
        let text_color = if face == Face::U || face == Face::D {
            Color32::BLACK
        } else {
            Color32::WHITE
        };
        p.text(
            rect.center(),
            egui::Align2::CENTER_CENTER,
            face.letter(),
            egui::FontId::proportional(30.0),
            text_color,
        );
        if response.hovered() {
            p.rect_stroke(
                rect,
                12.0,
                egui::Stroke::new(2.5, Color32::WHITE),
                egui::StrokeKind::Outside,
            );
        }
        if response.clicked() {
            // Plain tap: clockwise. Secondary (long-press/right-click): ccw.
            let m = Move::Face(face, Turns::Cw);
            app.history.push(m);
            app.animator.enqueue(m);
        }
        if response.secondary_clicked() {
            let m = Move::Face(face, Turns::Ccw);
            app.history.push(m);
            app.animator.enqueue(m);
        }
        let _ = Rect::NOTHING;
    }
}
