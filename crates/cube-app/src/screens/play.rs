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
        dim_others: 1.0,
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

/// Two big arrows that turn the tap-selected layer (animated). Always
/// visible (dull until a layer is tapped), and VIEW-RELATIVE: the part of
/// the layer nearest you moves in the arrow's screen direction, however
/// the cube is oriented. The caption shows the real notation (U, U', R…).
pub fn turn_arrows(app: &mut RubiksApp, ui: &mut Ui) {
    turn_arrows_sized(app, ui, Vec2::new(96.0, 76.0))
}

pub fn turn_arrows_sized(app: &mut RubiksApp, ui: &mut Ui, size: Vec2) {
    use cube_render::ArrowDir;
    let mapping = app
        .selected_face
        .map(|f| cube_render::view_relative_arrows(&app.orbit, f));
    let fill = if mapping.is_some() {
        Color32::from_rgb(0x2A, 0x5C, 0xC2)
    } else {
        Color32::from_gray(42)
    };
    let icon_for = |dir: ArrowDir| -> fn(&egui::Painter, egui::Rect) {
        match dir {
            ArrowDir::Left => icons::draw_back_arrow,
            ArrowDir::Right => icons::draw_next_arrow,
            ArrowDir::Up => icons::draw_tilt_up,
            ArrowDir::Down => icons::draw_tilt_down,
        }
    };
    let idle = [(ArrowDir::Left, Turns::Ccw), (ArrowDir::Right, Turns::Cw)];
    for (dir, turns) in mapping.unwrap_or(idle) {
        let caption = match app.selected_face {
            Some(face) => Move::Face(face, turns).to_string(),
            None => String::new(),
        };
        let response = icons::big_icon_button(ui, size, fill, &caption, icon_for(dir));
        if mapping.is_none() {
            // Dusk the whole button (icon included) while out of focus.
            ui.painter()
                .rect_filled(response.rect, 18.0, Color32::from_black_alpha(110));
        }
        if response.clicked() {
            if let Some(face) = app.selected_face {
                let m = Move::Face(face, turns);
                app.history.push(m);
                app.animator.enqueue(m);
            }
        }
    }
}

pub fn top_bar(app: &mut RubiksApp, ui: &mut Ui) {
    if top_bar_clicked(ui) {
        app.screen = Screen::Menu;
    }
}

pub enum TopBarAction {
    None,
    Back,
    Gear,
}

/// Top bar with back arrow AND a settings gear (solve screens): back is
/// hierarchical, the gear opens solution settings over the current
/// screen (state preserved).
pub fn top_bar_with_gear(ui: &mut Ui) -> TopBarAction {
    let mut action = TopBarAction::None;
    egui::Panel::top("topbar").show_separator_line(false).show(ui, |ui| {
        ui.horizontal(|ui| {
            let (rect, response) =
                ui.allocate_exact_size(Vec2::new(56.0, 40.0), Sense::click());
            ui.painter()
                .rect_filled(rect, 10.0, Color32::from_black_alpha(120));
            icons::draw_back_arrow(ui.painter(), rect.shrink2(Vec2::new(14.0, 10.0)));
            if response.clicked() {
                action = TopBarAction::Back;
            }
            ui.with_layout(egui::Layout::right_to_left(egui::Align::Min), |ui| {
                let (rect, resp) =
                    ui.allocate_exact_size(Vec2::new(44.0, 40.0), Sense::click());
                icons::draw_gear(ui.painter(), rect);
                if resp.clicked() {
                    action = TopBarAction::Gear;
                }
            });
        });
    });
    action
}

/// Draw the top-left back arrow; returns true on click so each screen can
/// go ONE level back (menu is only the default).
pub fn top_bar_clicked(ui: &mut Ui) -> bool {
    let mut clicked = false;
    egui::Panel::top("topbar").show_separator_line(false).show(ui, |ui| {
        ui.horizontal(|ui| {
            let (rect, response) =
                ui.allocate_exact_size(Vec2::new(56.0, 40.0), Sense::click());
            // Chip behind the white arrow so it reads on light theme too.
            ui.painter()
                .rect_filled(rect, 10.0, Color32::from_black_alpha(120));
            icons::draw_back_arrow(ui.painter(), rect.shrink2(Vec2::new(14.0, 10.0)));
            clicked = response.clicked();
        });
    });
    clicked
}

