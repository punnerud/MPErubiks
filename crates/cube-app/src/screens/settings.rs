//! Solution settings (the gear): hint mode, max extra moves, and the
//! priority-ordered list of algorithms woven into guided solutions.
//! Full screen (the app has no modals); back returns to the exact
//! screen the gear was opened from, state intact.

use crate::app::{HintMode, RubiksApp, Screen};
use crate::i18n::TextKey;
use crate::widgets::icons;
use cube_core::{CaseSet, RecogKind};
use egui::{Color32, RichText, ScrollArea, Sense, Ui, Vec2};

#[derive(Clone, Copy, PartialEq, Eq)]
pub enum SettingsFocus {
    /// From the front page / Solve: guide behaviour and priority.
    General,
    /// From Play: which algorithms to drill, and how the practice
    /// shuffle is built.
    Play,
}

pub struct SettingsScreen {
    /// The screen the gear was opened from — back restores it untouched.
    pub prev: Box<Screen>,
    pub focus: SettingsFocus,
}

fn set_badge(set: CaseSet) -> (&'static str, Color32) {
    match set {
        CaseSet::Pll => ("PLL", Color32::from_rgb(0xC7, 0x51, 0x08)),
        CaseSet::Oll => ("OLL", Color32::from_rgb(0x8E, 0x36, 0xB8)),
        CaseSet::F2l => ("F2L", Color32::from_rgb(0x2A, 0x5C, 0xC2)),
        CaseSet::Lbl => ("ABC", Color32::from_rgb(0x1E, 0x88, 0x50)),
    }
}

pub fn show(app: &mut RubiksApp, ui: &mut Ui) {
    let back_clicked = super::play::top_bar_clicked(ui);
    let Screen::Settings(screen) = std::mem::replace(&mut app.screen, Screen::Menu) else {
        return;
    };
    if back_clicked {
        app.save_hint_settings();
        app.screen = *screen.prev;
        return;
    }

    let mut dirty = false;
    let play_focus = screen.focus == SettingsFocus::Play;
    ui.vertical_centered(|ui| {
        let title = if play_focus {
            app.t(TextKey::PracticeShuffle)
        } else {
            app.t(TextKey::SettingsTitle)
        };
        ui.label(RichText::new(title).size(24.0).strong());
        if play_focus {
            ui.label(
                RichText::new(app.t(TextKey::PlayAffectsGuide))
                    .size(14.0)
                    .weak(),
            );
        }
    });
    ScrollArea::vertical().show(ui, |ui| {
        ui.vertical_centered(|ui| {
            if play_focus {
                // --- Scramble mode chips ---
                ui.add_space(8.0);
                ui.horizontal(|ui| {
                    ui.add_space((ui.available_width() - 3.0 * 128.0).max(0.0) / 2.0);
                    let modes = [
                        (cube_solver::ScrambleMode::Random, TextKey::ScrambleModeRandom),
                        (cube_solver::ScrambleMode::Built, TextKey::ScrambleModeBuilt),
                        (cube_solver::ScrambleMode::Auto, TextKey::ScrambleModeAuto),
                    ];
                    for (mode, label) in modes {
                        let active = app.play.mode == mode;
                        let color = if active {
                            Color32::from_rgb(0x8E, 0x36, 0xB8)
                        } else {
                            Color32::from_gray(60)
                        };
                        let (rect, resp) =
                            ui.allocate_exact_size(Vec2::new(116.0, 52.0), Sense::click());
                        ui.painter().rect_filled(rect, 12.0, color);
                        ui.painter().text(
                            rect.center(),
                            egui::Align2::CENTER_CENTER,
                            app.t(label),
                            egui::FontId::proportional(16.0),
                            Color32::WHITE,
                        );
                        if resp.clicked() {
                            app.play.mode = mode;
                            dirty = true;
                        }
                    }
                });
                // --- How many times ---
                ui.add_space(10.0);
                ui.label(RichText::new(app.t(TextKey::HowManyTimes)).size(18.0).weak());
                ui.horizontal(|ui| {
                    ui.add_space((ui.available_width() - 2.0 * 64.0 - 72.0).max(0.0) / 2.0);
                    let size = Vec2::new(64.0, 52.0);
                    if icons::big_icon_button(ui, size, Color32::from_gray(60), "", icons::draw_chevrons_left)
                        .clicked()
                        && app.play.target > 1
                    {
                        app.play.target -= 1;
                        dirty = true;
                    }
                    let (rect, _) = ui.allocate_exact_size(Vec2::new(72.0, 52.0), Sense::hover());
                    ui.painter().text(
                        rect.center(),
                        egui::Align2::CENTER_CENTER,
                        format!("×{}", app.play.target),
                        egui::FontId::proportional(28.0),
                        ui.visuals().strong_text_color(),
                    );
                    if icons::big_icon_button(ui, size, Color32::from_gray(60), "", icons::draw_chevrons_right)
                        .clicked()
                        && app.play.target < 3
                    {
                        app.play.target += 1;
                        dirty = true;
                    }
                });
                // --- Which algorithms to drill (recognizable ones) ---
                ui.add_space(12.0);
                ui.label(RichText::new(app.t(TextKey::PickAlgorithms)).size(18.0).weak());
                let cases: Vec<u16> = (0..app.library.rec.defs().len() as u16)
                    .filter(|&c| {
                        !matches!(app.library.rec.case(c).recognition, RecogKind::None)
                    })
                    .collect();
                for case_idx in cases {
                    let def = app.library.rec.case(case_idx);
                    let name = def.name.clone();
                    let (badge, badge_color) = set_badge(def.set);
                    let chosen = app.play.include.contains(&case_idx);
                    ui.horizontal(|ui| {
                        let w = 340.0f32.min(ui.available_width() - 16.0);
                        ui.add_space((ui.available_width() - w).max(0.0) / 2.0);
                        let (rect, resp) =
                            ui.allocate_exact_size(Vec2::new(w, 44.0), Sense::click());
                        let p = ui.painter();
                        p.rect_filled(
                            rect,
                            10.0,
                            if chosen {
                                Color32::from_rgb(0x8E, 0x36, 0xB8)
                            } else {
                                Color32::from_gray(34)
                            },
                        );
                        let badge_rect = egui::Rect::from_center_size(
                            rect.left_center() + Vec2::new(34.0, 0.0),
                            Vec2::new(48.0, 24.0),
                        );
                        p.rect_filled(badge_rect, 6.0, badge_color);
                        p.text(
                            badge_rect.center(),
                            egui::Align2::CENTER_CENTER,
                            badge,
                            egui::FontId::proportional(13.0),
                            Color32::WHITE,
                        );
                        p.text(
                            rect.left_center() + Vec2::new(68.0, 0.0),
                            egui::Align2::LEFT_CENTER,
                            name,
                            egui::FontId::proportional(16.0),
                            Color32::from_gray(235),
                        );
                        if resp.clicked() {
                            app.toggle_play_algorithm(case_idx);
                        }
                    });
                }
                ui.add_space(crate::app::BOTTOM_INSET);
                return;
            }
            // --- Hint mode: three chips ---
            ui.add_space(8.0);
            ui.horizontal(|ui| {
                ui.add_space((ui.available_width() - 3.0 * 128.0).max(0.0) / 2.0);
                let modes = [
                    (HintMode::Full, TextKey::HintModeFull),
                    (HintMode::Practice, TextKey::HintModePractice),
                    (HintMode::Off, TextKey::HintModeOff),
                ];
                for (mode, label) in modes {
                    let active = app.hints.mode == mode;
                    let color = if active {
                        Color32::from_rgb(0x1E, 0x88, 0x50)
                    } else {
                        Color32::from_gray(60)
                    };
                    let (rect, resp) =
                        ui.allocate_exact_size(Vec2::new(116.0, 52.0), Sense::click());
                    ui.painter().rect_filled(rect, 12.0, color);
                    ui.painter().text(
                        rect.center(),
                        egui::Align2::CENTER_CENTER,
                        app.t(label),
                        egui::FontId::proportional(16.0),
                        Color32::WHITE,
                    );
                    if resp.clicked() {
                        app.hints.mode = mode;
                        dirty = true;
                    }
                }
            });

            // --- Max extra moves: [-] n [+] ---
            ui.add_space(10.0);
            ui.label(RichText::new(app.t(TextKey::MaxExtraMoves)).size(18.0).weak());
            ui.horizontal(|ui| {
                ui.add_space((ui.available_width() - 2.0 * 64.0 - 72.0).max(0.0) / 2.0);
                let size = Vec2::new(64.0, 52.0);
                if icons::big_icon_button(
                    ui,
                    size,
                    Color32::from_gray(60),
                    "",
                    icons::draw_chevrons_left,
                )
                .clicked()
                    && app.hints.max_extra > 0
                {
                    app.hints.max_extra -= 1;
                    dirty = true;
                }
                let (rect, _) = ui.allocate_exact_size(Vec2::new(72.0, 52.0), Sense::hover());
                ui.painter().text(
                    rect.center(),
                    egui::Align2::CENTER_CENTER,
                    app.hints.max_extra.to_string(),
                    egui::FontId::proportional(30.0),
                    ui.visuals().strong_text_color(),
                );
                if icons::big_icon_button(
                    ui,
                    size,
                    Color32::from_gray(60),
                    "",
                    icons::draw_chevrons_right,
                )
                .clicked()
                    && app.hints.max_extra < 12
                {
                    app.hints.max_extra += 1;
                    dirty = true;
                }
            });

            // --- Tap-select mode: side vs cell ---
            ui.add_space(10.0);
            ui.label(RichText::new(app.t(TextKey::TapSelectTitle)).size(18.0).weak());
            ui.horizontal(|ui| {
                ui.add_space((ui.available_width() - 2.0 * 128.0).max(0.0) / 2.0);
                let opts = [
                    (false, TextKey::TapSelectSide),
                    (true, TextKey::TapSelectCell),
                ];
                for (cell, label) in opts {
                    let active = app.tap_cell == cell;
                    let color = if active {
                        Color32::from_rgb(0x1E, 0x88, 0x50)
                    } else {
                        Color32::from_gray(60)
                    };
                    let (rect, resp) =
                        ui.allocate_exact_size(Vec2::new(116.0, 52.0), Sense::click());
                    ui.painter().rect_filled(rect, 12.0, color);
                    ui.painter().text(
                        rect.center(),
                        egui::Align2::CENTER_CENTER,
                        app.t(label),
                        egui::FontId::proportional(16.0),
                        Color32::WHITE,
                    );
                    if resp.clicked() {
                        app.tap_cell = cell;
                        dirty = true;
                    }
                }
            });

            // --- Priority list: trained algorithms only, flat order ---
            ui.add_space(12.0);
            let include = app.hints.include.clone();
            if include.is_empty() {
                ui.label(
                    RichText::new(app.t(TextKey::SettingsNoTrained))
                        .size(18.0)
                        .weak(),
                );
            }
            for (row, &case_idx) in include.iter().enumerate() {
                let def = app.library.rec.case(case_idx);
                let name = def.name.clone();
                let (badge, badge_color) = set_badge(def.set);
                ui.horizontal(|ui| {
                    let total_w = 358.0f32.min(ui.available_width() - 16.0);
                    ui.add_space((ui.available_width() - total_w).max(0.0) / 2.0);
                    // Include toggle: tap the star to EXCLUDE from guides
                    // (the ★ in Train still means "I know this").
                    let (star, star_resp) =
                        ui.allocate_exact_size(Vec2::new(40.0, 48.0), Sense::click());
                    ui.painter().text(
                        star.center(),
                        egui::Align2::CENTER_CENTER,
                        "★",
                        egui::FontId::proportional(26.0),
                        Color32::from_rgb(0xFF, 0xD5, 0x00),
                    );
                    if star_resp.clicked() {
                        app.hints.include.retain(|&c| c != case_idx);
                        dirty = true;
                    }
                    // Priority number + set badge + name.
                    let (rect, _) = ui.allocate_exact_size(
                        Vec2::new(total_w - 2.0 * 56.0 - 48.0, 48.0),
                        Sense::hover(),
                    );
                    let p = ui.painter();
                    p.rect_filled(rect, 10.0, Color32::from_gray(38));
                    p.text(
                        rect.left_center() + Vec2::new(14.0, 0.0),
                        egui::Align2::LEFT_CENTER,
                        format!("{}", row + 1),
                        egui::FontId::proportional(20.0),
                        Color32::from_gray(140),
                    );
                    let badge_rect = egui::Rect::from_center_size(
                        rect.left_center() + Vec2::new(62.0, 0.0),
                        Vec2::new(48.0, 26.0),
                    );
                    p.rect_filled(badge_rect, 6.0, badge_color);
                    p.text(
                        badge_rect.center(),
                        egui::Align2::CENTER_CENTER,
                        badge,
                        egui::FontId::proportional(13.0),
                        Color32::WHITE,
                    );
                    p.text(
                        rect.left_center() + Vec2::new(96.0, 0.0),
                        egui::Align2::LEFT_CENTER,
                        name,
                        egui::FontId::proportional(17.0),
                        Color32::from_gray(230),
                    );
                    // Up / down.
                    let can_up = row > 0;
                    let can_down = row + 1 < include.len();
                    if icons::big_icon_button(
                        ui,
                        Vec2::new(48.0, 48.0),
                        if can_up {
                            Color32::from_gray(60)
                        } else {
                            Color32::from_gray(42)
                        },
                        "",
                        icons::draw_tilt_up,
                    )
                    .clicked()
                        && can_up
                    {
                        app.hints.include.swap(row, row - 1);
                        dirty = true;
                    }
                    if icons::big_icon_button(
                        ui,
                        Vec2::new(48.0, 48.0),
                        if can_down {
                            Color32::from_gray(60)
                        } else {
                            Color32::from_gray(42)
                        },
                        "",
                        icons::draw_tilt_down,
                    )
                    .clicked()
                        && can_down
                    {
                        app.hints.include.swap(row, row + 1);
                        dirty = true;
                    }
                });
            }
            // Trained but EXCLUDED: dimmed rows, tap the star to re-
            // include (appended at lowest priority).
            let mut excluded: Vec<u16> = app
                .trained
                .iter()
                .copied()
                .filter(|c| !app.hints.include.contains(c))
                .collect();
            excluded.sort_unstable();
            for case_idx in excluded {
                let def = app.library.rec.case(case_idx);
                let name = def.name.clone();
                let (badge, badge_color) = set_badge(def.set);
                ui.horizontal(|ui| {
                    let total_w = 358.0f32.min(ui.available_width() - 16.0);
                    ui.add_space((ui.available_width() - total_w).max(0.0) / 2.0);
                    let (star, star_resp) =
                        ui.allocate_exact_size(Vec2::new(40.0, 48.0), Sense::click());
                    ui.painter().text(
                        star.center(),
                        egui::Align2::CENTER_CENTER,
                        "☆",
                        egui::FontId::proportional(26.0),
                        Color32::from_gray(110),
                    );
                    if star_resp.clicked() {
                        app.hints.include.push(case_idx);
                        dirty = true;
                    }
                    let (rect, _) = ui.allocate_exact_size(
                        Vec2::new(total_w - 2.0 * 56.0 - 48.0, 48.0),
                        Sense::hover(),
                    );
                    let p = ui.painter();
                    p.rect_filled(rect, 10.0, Color32::from_gray(30));
                    let badge_rect = egui::Rect::from_center_size(
                        rect.left_center() + Vec2::new(38.0, 0.0),
                        Vec2::new(48.0, 26.0),
                    );
                    p.rect_filled(badge_rect, 6.0, badge_color.gamma_multiply(0.4));
                    p.text(
                        badge_rect.center(),
                        egui::Align2::CENTER_CENTER,
                        badge,
                        egui::FontId::proportional(13.0),
                        Color32::from_gray(160),
                    );
                    p.text(
                        rect.left_center() + Vec2::new(72.0, 0.0),
                        egui::Align2::LEFT_CENTER,
                        name,
                        egui::FontId::proportional(17.0),
                        Color32::from_gray(120),
                    );
                });
            }
            ui.add_space(crate::app::BOTTOM_INSET);
        });
    });
    if dirty {
        app.save_hint_settings();
    }
    app.screen = Screen::Settings(screen);
}
