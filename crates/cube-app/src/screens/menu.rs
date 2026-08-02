//! Front page: giant visual mode buttons + language flags. Designed for
//! children who can't read yet — each mode has a distinct icon and color.

use crate::app::{RubiksApp, Screen};
use crate::i18n::{Lang, TextKey};
use crate::widgets::{flags, icons};
use egui::{Color32, Rect, Ui, Vec2};

pub fn show(app: &mut RubiksApp, ui: &mut Ui) {
    // Theme toggle top left, gear + language flags top right — all on
    // the SAME center line (fixed 40px row, center-aligned cluster).
    ui.horizontal(|ui| {
        ui.set_min_size(Vec2::new(ui.available_width(), 40.0));
        let (rect, resp) =
            ui.allocate_exact_size(Vec2::new(44.0, 40.0), egui::Sense::click());
        let p = ui.painter();
        let c = rect.center();
        if app.light_mode {
            // Moon: tap to go dark.
            let r = 13.0;
            p.circle_filled(c, r, egui::Color32::from_rgb(0x3A, 0x42, 0x55));
            p.circle_filled(
                c + Vec2::new(5.0, -4.0),
                r * 0.85,
                ui.visuals().panel_fill,
            );
        } else {
            // Sun: tap to go light.
            let r = 9.0;
            let col = egui::Color32::from_rgb(0xFF, 0xD5, 0x00);
            p.circle_filled(c, r, col);
            for i in 0..8 {
                let a = i as f32 * std::f32::consts::TAU / 8.0;
                let d = Vec2::new(a.cos(), a.sin());
                p.line_segment([c + d * (r + 3.0), c + d * (r + 7.0)], egui::Stroke::new(2.5, col));
            }
        }
        if resp.clicked() {
            app.light_mode = !app.light_mode;
            crate::app::apply_theme(ui.ctx(), app.light_mode);
            if let Some(store) = &app.store {
                let _ = store.set_setting(
                    "theme",
                    if app.light_mode { "light" } else { "dark" },
                );
                crate::persist::persist(store);
            }
        }
        ui.with_layout(egui::Layout::right_to_left(egui::Align::Center), |ui| {
            // The CURRENT language's flag; tapping it opens the picker.
            if flags::flag_button(ui, app.i18n.lang, false) {
                let prev = std::mem::replace(&mut app.screen, crate::app::Screen::Menu);
                app.screen = crate::app::Screen::Language(
                    crate::screens::language::LanguageScreen { prev: Box::new(prev) },
                );
            }
            // Solution settings (gear).
            let (rect, resp) =
                ui.allocate_exact_size(Vec2::new(44.0, 40.0), egui::Sense::click());
            icons::draw_gear(ui.painter(), rect);
            if resp.clicked() {
                let prev = std::mem::replace(&mut app.screen, crate::app::Screen::Menu);
                app.screen = crate::app::Screen::Settings(
                    crate::screens::settings::SettingsScreen { prev: Box::new(prev) },
                );
            }
        });
    });

    ui.vertical_centered(|ui| {
        ui.add_space(12.0);
        // Logo: painter-drawn scrambled mini-cube + title.
        let (logo_rect, _) = ui.allocate_exact_size(Vec2::splat(72.0), egui::Sense::hover());
        icons::draw_mini_cube(ui.painter(), logo_rect.shrink(4.0), icons::scrambled_face());
        ui.heading(app.t(TextKey::AppTitle));
        ui.add_space(20.0);

        let button_size = button_size(ui);
        if icons::big_icon_button(
            ui,
            button_size,
            Color32::from_rgb(0x1E, 0x88, 0x50),
            app.t(TextKey::MenuSolve),
            |p, r| {
                // Solve = light bulb + SOLVED 3D cube, vertically
                // centered with each other.
                let side = r.height().min(r.width() * 0.46);
                let left_sq = Rect::from_center_size(
                    egui::Pos2::new(r.left() + r.width() * 0.27, r.center().y),
                    Vec2::splat(side),
                );
                let right_sq = Rect::from_center_size(
                    egui::Pos2::new(r.left() + r.width() * 0.73, r.center().y),
                    Vec2::splat(side),
                );
                icons::draw_bulb(p, left_sq);
                icons::draw_iso_cube(p, right_sq);
            },
        )
        .clicked()
        {
            #[cfg(target_arch = "wasm32")]
            {
                app.screen =
                    Screen::Scan(crate::screens::scan::ScanScreen::new(app, ui.ctx()));
            }
            #[cfg(not(target_arch = "wasm32"))]
            {
                app.screen =
                    Screen::Solve(crate::screens::solve::SolveScreen::new_input(app.cube));
            }
        }

        if icons::big_icon_button(
            ui,
            button_size,
            Color32::from_rgb(0xC7, 0x51, 0x08),
            app.t(TextKey::MenuTrain),
            icons::draw_dumbbell,
        )
        .clicked()
        {
            app.screen = Screen::Train(crate::screens::train::TrainScreen::Picker {
                tab: crate::screens::train::PickerTab::Intro,
            });
        }

        if icons::big_icon_button(
            ui,
            button_size,
            Color32::from_rgb(0x2A, 0x5C, 0xC2),
            app.t(TextKey::MenuPlay),
            |p, r| icons::draw_mini_cube(p, r, icons::scrambled_face()),
        )
        .clicked()
        {
            app.screen = Screen::Play;
        }
    });
}

fn button_size(ui: &Ui) -> Vec2 {
    let w = (ui.available_width() * 0.8).clamp(220.0, 420.0);
    Vec2::new(w, 110.0)
}
