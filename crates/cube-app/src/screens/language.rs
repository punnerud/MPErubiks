//! Language picker: a scrollable list of flags and native names. Tap the
//! flag on the front page to open it. Everything here is language-free
//! by construction — a child recognizes the flag, a reader recognizes
//! their language's own name.

use crate::app::{RubiksApp, Screen};
use crate::i18n::{Lang, LANGS};
use crate::widgets::flags;
use egui::{Color32, ScrollArea, Sense, Stroke, StrokeKind, Ui, Vec2};

pub struct LanguageScreen {
    /// The screen to return to (the picker is opened over it).
    pub prev: Box<Screen>,
    /// Set when a language is picked: the list closes itself shortly
    /// after, so the choice is confirmed on screen without a second tap.
    pub close_at: Option<f64>,
}

pub fn show(app: &mut RubiksApp, ui: &mut Ui) {
    let back_clicked = super::play::top_bar_clicked(ui);
    let Screen::Language(mut screen) = std::mem::replace(&mut app.screen, Screen::Menu) else {
        return;
    };
    let now = ui.input(|i| i.time);
    if back_clicked || screen.close_at.is_some_and(|t| now >= t) {
        app.screen = *screen.prev;
        return;
    }
    if screen.close_at.is_some() {
        ui.ctx().request_repaint();
    }
    // Opening the picker is the moment to fetch the script fonts: the
    // list itself wants to show 简体中文 / 日本語 / 한국어 in their own
    // script. Nothing was downloaded before the user asked for this
    // screen, and each subset is a few dozen KB.
    for (i, def) in LANGS.iter().enumerate() {
        if def.font.is_some() {
            app.ensure_font(Lang(i as u8), ui.ctx());
        }
    }
    let mut chosen: Option<Lang> = None;
    let current = app.i18n.lang;
    ScrollArea::vertical().show(ui, |ui| {
        ui.vertical_centered(|ui| {
            for (i, def) in LANGS.iter().enumerate() {
                let lang = Lang(i as u8);
                let active = lang == current;
                let row_w = 320.0f32.min(ui.available_width() - 16.0);
                let (rect, resp) =
                    ui.allocate_exact_size(Vec2::new(row_w, 52.0), Sense::click());
                let p = ui.painter();
                p.rect_filled(
                    rect,
                    10.0,
                    if active {
                        Color32::from_rgb(0x1E, 0x88, 0x50)
                    } else {
                        Color32::from_gray(38)
                    },
                );
                if resp.hovered() && !active {
                    p.rect_stroke(
                        rect,
                        10.0,
                        Stroke::new(1.5, Color32::from_gray(120)),
                        StrokeKind::Inside,
                    );
                }
                let flag_rect = egui::Rect::from_center_size(
                    egui::Pos2::new(rect.left() + 34.0, rect.center().y),
                    Vec2::new(34.0, 24.0),
                );
                let row_bg = if active {
                    Color32::from_rgb(0x1E, 0x88, 0x50)
                } else {
                    Color32::from_gray(38)
                };
                flags::draw_flag_rounded(p, flag_rect, def.flag, row_bg);
                p.rect_stroke(
                    flag_rect,
                    3.0,
                    Stroke::new(1.0, Color32::from_black_alpha(90)),
                    StrokeKind::Outside,
                );
                match def.latin {
                    // Two lines while a downloaded script is involved:
                    // the native name, and a Latin name that is legible
                    // even before the font subset lands.
                    Some(latin) => {
                        p.text(
                            egui::Pos2::new(rect.left() + 64.0, rect.center().y - 9.0),
                            egui::Align2::LEFT_CENTER,
                            def.native,
                            egui::FontId::proportional(19.0),
                            Color32::from_gray(240),
                        );
                        p.text(
                            egui::Pos2::new(rect.left() + 64.0, rect.center().y + 11.0),
                            egui::Align2::LEFT_CENTER,
                            latin,
                            egui::FontId::proportional(13.0),
                            Color32::from_gray(160),
                        );
                    }
                    None => {
                        p.text(
                            egui::Pos2::new(rect.left() + 64.0, rect.center().y),
                            egui::Align2::LEFT_CENTER,
                            def.native,
                            egui::FontId::proportional(20.0),
                            Color32::from_gray(240),
                        );
                    }
                }
                // A dot marks languages whose script arrives as a small
                // font download the first time they are picked.
                if def.font.is_some() {
                    p.circle_filled(
                        egui::Pos2::new(rect.right() - 18.0, rect.center().y),
                        4.0,
                        Color32::from_gray(120),
                    );
                }
                if resp.clicked() {
                    chosen = Some(lang);
                }
            }
            ui.add_space(crate::app::BOTTOM_INSET);
        });
    });
    if let Some(lang) = chosen {
        app.set_lang(lang, ui.ctx());
        // Show the new language applied (the whole UI switches under the
        // list), then close on its own.
        screen.close_at = Some(now + 0.7);
    }
    app.screen = Screen::Language(screen);
}
