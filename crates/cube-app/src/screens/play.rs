//! Play mode: free 3D cube with scramble/reset/undo and tap-to-turn face
//! buttons. Buttons are colored like the faces themselves so pre-readers
//! can connect button -> face.

use crate::app::{RubiksApp, Screen};
use crate::i18n::TextKey;
use crate::widgets::{cube_view::CubeView, icons};
use cube_core::{FaceletCube, Move, Turns};
use egui::{Color32, Sense, Ui, Vec2};

pub fn show(app: &mut RubiksApp, ui: &mut Ui) {
    // Gear here opens the SAME settings screen, focused on the practice
    // selection (which also feeds the solve guide).
    match top_bar_with_gear(ui) {
        TopBarAction::Back => {
            app.screen = Screen::Menu;
            return;
        }
        TopBarAction::Gear => {
            let prev = std::mem::replace(&mut app.screen, Screen::Menu);
            app.screen = Screen::Settings(crate::screens::settings::SettingsScreen {
                prev: Box::new(prev),
                focus: crate::screens::settings::SettingsFocus::Play,
            });
            return;
        }
        TopBarAction::None => {}
    }

    // Actions ABOVE the cube (thumb-reach + clear of the iOS bottom bar).
    ui.vertical_centered(|ui| {
        ui.horizontal(|ui| {
            ui.add_space((ui.available_width() - 3.0 * 92.0).max(0.0) / 2.0);
            action_buttons(app, ui);
        });
    });
    if app.last_practice.is_some() {
        // Drawn in flow: the cube below simply gets the remaining space.
        practice_card(app, ui);
    }

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
        hint: None,
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

/// Shuffle: a PRACTICE scramble when algorithms are selected (its
/// solution contains them), otherwise the plain random one.
fn practice_or_plain_scramble(app: &mut RubiksApp) {
    let wanted: Vec<u16> = app.play.include.clone();
    if wanted.is_empty() || !matches!(app.table, crate::app::TableState::Ready) {
        app.last_practice = None;
        app.scramble();
        return;
    }
    let mut rng = cube_core::SplitMix64::new(app.rng.next_u64());
    match cube_solver::practice_scramble(
        &mut rng,
        &app.library.rec,
        &wanted,
        app.play.target,
        app.play.mode,
        app.hints.max_extra,
    ) {
        Some(ps) => {
            app.animator.clear();
            app.cube = ps.state;
            app.history.clear();
            app.last_practice = Some(crate::app::PracticeInfo {
                counts: ps
                    .counts
                    .iter()
                    .map(|(idx, n)| (app.library.rec.case(*idx).name.clone(), *n))
                    .collect(),
                moves: ps.solution.total_htm,
                share: ps.share,
                state: ps.state,
                solution: ps.solution,
            });
        }
        // No practice scramble available (e.g. only beginner steps
        // chosen, which the weave engine cannot target): plain shuffle.
        None => {
            app.last_practice = None;
            app.scramble();
        }
    }
}

/// What the last practice shuffle produced: which algorithms, how many
/// times, how long the solution is, and how much of it is the
/// algorithms — with a bar showing that share.
fn practice_card(app: &mut RubiksApp, ui: &mut Ui) {
    let Some(info) = &app.last_practice else { return };
    let names: Vec<String> = info
        .counts
        .iter()
        .map(|(name, n)| {
            if *n > 1 {
                format!("{name} ×{n}")
            } else {
                name.clone()
            }
        })
        .collect();
    let title = format!("★ {}", names.join(" · "));
    let detail = format!(
        "{} {} · {:.0} % {}",
        info.moves,
        app.t(TextKey::Moves),
        info.share * 100.0,
        app.t(TextKey::ShareOfSolution)
    );
    let share = info.share.clamp(0.0, 1.0);
    let show_clicked = ui
        .vertical_centered(|ui| {
            let w = 320.0f32.min(ui.available_width() - 16.0);
            let (rect, resp) = ui.allocate_exact_size(Vec2::new(w, 70.0), Sense::click());
            let p = ui.painter();
            p.rect_filled(rect, 12.0, Color32::from_gray(34));
            p.text(
                egui::Pos2::new(rect.left() + 14.0, rect.top() + 18.0),
                egui::Align2::LEFT_CENTER,
                title,
                egui::FontId::proportional(18.0),
                Color32::from_rgb(0xFF, 0xD5, 0x00),
            );
            p.text(
                egui::Pos2::new(rect.left() + 14.0, rect.top() + 40.0),
                egui::Align2::LEFT_CENTER,
                detail,
                egui::FontId::proportional(14.0),
                Color32::from_gray(190),
            );
            // Share bar: the green part is the algorithms.
            let bar = egui::Rect::from_min_size(
                egui::Pos2::new(rect.left() + 14.0, rect.bottom() - 16.0),
                Vec2::new(rect.width() - 28.0, 6.0),
            );
            p.rect_filled(bar, 3.0, Color32::from_gray(60));
            p.rect_filled(
                egui::Rect::from_min_size(bar.min, Vec2::new(bar.width() * share, 6.0)),
                3.0,
                Color32::from_rgb(0x1E, 0x88, 0x50),
            );
            resp.clicked()
        })
        .inner;
    if show_clicked {
        if let Some(info) = app.last_practice.take() {
            let guide = crate::screens::solve::GuideState::from_guided(
                info.solution,
                &app.library.rec,
                info.state,
                matches!(app.hints.mode, crate::app::HintMode::Practice),
            );
            app.cube = info.state;
            app.animator.clear();
            app.screen = Screen::Solve(crate::screens::solve::SolveScreen::Guide(guide));
        }
    }
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
        practice_or_plain_scramble(app);
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
        app.last_practice = None;
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
            let hit = if app.tap_cell {
                cube_render::pick_layer_by_cell(&app.orbit, rect.aspect_ratio(), ndc)
            } else {
                cube_render::pick_face(&app.orbit, rect.aspect_ratio(), ndc)
            };
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

