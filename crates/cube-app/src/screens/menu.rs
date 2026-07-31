//! Front page: giant visual mode buttons + language flags. Designed for
//! children who can't read yet — each mode has a distinct icon and color.

use crate::app::{RubiksApp, Screen};
use crate::i18n::{Lang, TextKey};
use crate::widgets::{flags, icons};
use egui::{Color32, Rect, Ui, Vec2};

pub fn show(app: &mut RubiksApp, ui: &mut Ui) {
    // Language flags, top right.
    ui.horizontal(|ui| {
        ui.with_layout(egui::Layout::right_to_left(egui::Align::Min), |ui| {
            if flags::flag_button(ui, Lang::No, app.i18n.lang == Lang::No) {
                app.set_lang(Lang::No);
            }
            if flags::flag_button(ui, Lang::En, app.i18n.lang == Lang::En) {
                app.set_lang(Lang::En);
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
                // Solve = camera + solved face: scan your cube, get help.
                let half = Vec2::new(r.width() * 0.46, r.height());
                icons::draw_camera(p, Rect::from_min_size(r.min, half));
                icons::draw_mini_cube(
                    p,
                    Rect::from_min_size(
                        r.min + Vec2::new(r.width() * 0.54, 0.0),
                        half,
                    ),
                    icons::solved_face(Color32::from_rgb(0xFF, 0xD5, 0x00)),
                );
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
