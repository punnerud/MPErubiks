//! Training mode: pick a case (visual diagrams), drill it with a timer on
//! your physical cube, mark success/fail, watch your times improve.
//! Scores persist via cube-store from M6; until then they live in-app.

use crate::app::{RubiksApp, Screen};
use crate::i18n::TextKey;
use crate::widgets::{case_diagram, cube_view::CubeView, icons};
use cube_core::{CaseSet, RecogKind};
use egui::{Color32, Rect, RichText, ScrollArea, Sense, Stroke, StrokeKind, Ui, Vec2};

#[derive(Clone, Copy, PartialEq, Eq)]
pub enum PickerTab {
    /// Beginner lessons — the "start here" tab.
    Intro,
    Set(CaseSet),
}

pub enum TrainScreen {
    Picker { tab: PickerTab },
    Session(SessionState),
    Lesson(LessonView),
}

pub struct LessonView {
    pub lesson: usize,
    pub step: usize,
    /// The step's setup+demo still needs to be applied to the cube.
    pub pending: bool,
    /// Number of moves in the currently playing demo (karaoke progress).
    pub demo_len: usize,
    /// Moves of the demo applied so far (drip-fed like the solve guide).
    pub cursor: usize,
    pub playing: bool,
    /// Playback holds until this time: a 1s breath at start/restart so
    /// the eye finds the cube before colors start moving.
    pub play_at: f64,
}

pub struct SessionState {
    pub case_idx: u16,
    pub phase: Phase,
}

pub enum Phase {
    Ready,
    Timing { start: f64 },
    Result { ms: u64, success: Option<bool> },
}

#[derive(Clone, Copy)]
pub struct Attempt {
    pub ms: u64,
    pub success: bool,
}

pub fn show(app: &mut RubiksApp, ui: &mut Ui) {
    let back_clicked = super::play::top_bar_clicked(ui);
    let Screen::Train(mut screen) = std::mem::replace(&mut app.screen, Screen::Menu) else {
        return;
    };
    let mut next: Option<Screen> = None;
    // Hierarchical back: inside a lesson/session the top-left arrow goes
    // to the picker (on the matching tab), not to the front page.
    if back_clicked {
        next = Some(match &screen {
            TrainScreen::Picker { .. } => Screen::Menu,
            TrainScreen::Lesson(_) => Screen::Train(TrainScreen::Picker {
                tab: PickerTab::Intro,
            }),
            TrainScreen::Session(session) => {
                app.animator.clear();
                Screen::Train(TrainScreen::Picker {
                    tab: PickerTab::Set(app.library.rec.case(session.case_idx).set),
                })
            }
        });
    }

    match &mut screen {
        TrainScreen::Picker { tab } => picker(app, ui, tab, &mut next),
        TrainScreen::Session(session) => session_ui(app, ui, session, &mut next),
        TrainScreen::Lesson(view) => lesson_ui(app, ui, view, &mut next),
    }

    app.screen = next.unwrap_or(Screen::Train(screen));
}

fn set_tab_label(set: CaseSet) -> &'static str {
    match set {
        CaseSet::Pll => "PLL",
        CaseSet::Oll => "OLL",
        CaseSet::F2l => "F2L",
        CaseSet::Lbl => "ABC",
    }
}

fn picker(app: &mut RubiksApp, ui: &mut Ui, tab: &mut PickerTab, next: &mut Option<Screen>) {
    ui.vertical_centered(|ui| {
        ui.horizontal(|ui| {
            ui.add_space((ui.available_width() - 5.0 * 96.0).max(0.0) / 2.0);
            let tabs = [
                (PickerTab::Intro, "123"),
                (PickerTab::Set(CaseSet::Lbl), "ABC"),
                (PickerTab::Set(CaseSet::F2l), "F2L"),
                (PickerTab::Set(CaseSet::Oll), "OLL"),
                (PickerTab::Set(CaseSet::Pll), "PLL"),
            ];
            for (t, label) in tabs {
                let active = *tab == t;
                let color = if active {
                    if t == PickerTab::Intro {
                        Color32::from_rgb(0x1E, 0x88, 0x50)
                    } else {
                        Color32::from_rgb(0xC7, 0x51, 0x08)
                    }
                } else {
                    Color32::from_gray(60)
                };
                let (rect, response) =
                    ui.allocate_exact_size(Vec2::new(84.0, 48.0), Sense::click());
                ui.painter().rect_filled(rect, 12.0, color);
                ui.painter().text(
                    rect.center(),
                    egui::Align2::CENTER_CENTER,
                    label,
                    egui::FontId::proportional(22.0),
                    Color32::WHITE,
                );
                if response.clicked() {
                    *tab = t;
                }
            }
        });
    });
    ui.add_space(8.0);

    match *tab {
        PickerTab::Intro => intro_picker(app, ui, next),
        PickerTab::Set(set) => {
            let cases = app.library.cases_in_set(set);
            ScrollArea::vertical().show(ui, |ui| {
                ui.horizontal_wrapped(|ui| {
                    for case_idx in cases {
                        if case_card(app, ui, case_idx) {
                            new_case_state(app, case_idx);
                            *next = Some(Screen::Train(TrainScreen::Session(SessionState {
                                case_idx,
                                phase: Phase::Ready,
                            })));
                        }
                    }
                });
            });
        }
    }
}

/// Lesson cards: a huge number badge + short title. Ordered 1..7 so a
/// child can follow along without reading.
fn intro_picker(app: &mut RubiksApp, ui: &mut Ui, next: &mut Option<Screen>) {
    ui.vertical_centered(|ui| {
        ui.label(RichText::new(app.t(TextKey::LessonsTabCaption)).size(20.0).weak());
    });
    ScrollArea::vertical().show(ui, |ui| {
        ui.horizontal_wrapped(|ui| {
            for (i, lesson) in crate::lessons::LESSONS.iter().enumerate() {
                let size = Vec2::new(132.0, 150.0);
                let (rect, response) = ui.allocate_exact_size(size, Sense::click());
                let p = ui.painter();
                p.rect_filled(rect, 12.0, Color32::from_gray(38));
                if response.hovered() {
                    p.rect_stroke(rect, 12.0, Stroke::new(2.0, Color32::WHITE), StrokeKind::Inside);
                }
                p.text(
                    egui::Pos2::new(rect.center().x, rect.top() + 52.0),
                    egui::Align2::CENTER_CENTER,
                    lesson.badge,
                    egui::FontId::proportional(56.0),
                    Color32::from_rgb(0x4C, 0xD9, 0x64),
                );
                p.text(
                    egui::Pos2::new(rect.center().x, rect.bottom() - 26.0),
                    egui::Align2::CENTER_CENTER,
                    app.t(lesson.title),
                    egui::FontId::proportional(15.0),
                    Color32::WHITE,
                );
                if response.clicked() {
                    *next = Some(Screen::Train(TrainScreen::Lesson(LessonView {
                        lesson: i,
                        step: 0,
                        pending: true,
                        demo_len: 0,
                        cursor: 0,
                        playing: false,
                        play_at: 0.0,
                    })));
                }
            }
        });
    });
}

/// One lesson step: the cube DEMONSTRATES, one big sentence reinforces,
/// giant replay/next controls.
fn lesson_ui(app: &mut RubiksApp, ui: &mut Ui, view: &mut LessonView, next: &mut Option<Screen>) {
    let now = ui.input(|i| i.time);
    let lesson = &crate::lessons::LESSONS[view.lesson];
    let step = &lesson.steps[view.step];
    let demo_moves: Vec<cube_core::Move> = step
        .demo
        .and_then(|d| cube_core::Alg::parse(d).ok())
        .map(|a| a.0)
        .unwrap_or_default();

    if view.pending {
        view.pending = false;
        app.animator.clear();
        // Calm demo tempo at every step start: fast playback makes it
        // impossible to see where the colors come from. The speed
        // buttons still adjust it during the step.
        app.animator.secs_per_quarter = 0.6;
        app.selected_face = None;
        // Steps CHAIN: only jump-reset the cube when the state actually
        // differs from the step's starting point (an invisible rewind
        // that replays into the same place reads as a glitch).
        let mut target = cube_core::FaceletCube::SOLVED;
        if let Some(setup) = step.setup {
            if let Ok(alg) = cube_core::Alg::parse(setup) {
                target.apply_alg(&alg);
            }
        }
        if app.cube != target {
            app.cube = target;
        }
        view.demo_len = demo_moves.len();
        view.cursor = 0;
        // `playing` is left as the caller set it: false on step entry
        // (the Play button starts the demo), true on Restart (replay
        // with the same 1s breath).
        view.play_at = now + 1.0;
    }

    // Drip-feed one move at a time (same model as the solve guide): pause
    // and single-stepping stay exact, and the karaoke row tracks reality.
    if view.playing
        && now >= view.play_at
        && view.cursor < demo_moves.len()
        && app.animator.is_idle()
    {
        app.animator.enqueue(demo_moves[view.cursor]);
        view.cursor += 1;
    }
    // egui only paints on input or explicit request: without this, the
    // 1s pre-roll (and the gap between moves) waits forever for a touch.
    if view.playing && view.cursor < demo_moves.len() {
        ui.ctx().request_repaint();
    }
    // Demo finished: if it ended exactly where it began (cyclic
    // sequence), arm Play directly - a Restart to the identical state
    // is pointless.
    if view.playing && view.cursor >= demo_moves.len() && app.animator.is_idle() {
        view.playing = false;
        let mut start = cube_core::FaceletCube::SOLVED;
        if let Some(setup) = step.setup {
            if let Ok(alg) = cube_core::Alg::parse(setup) {
                start.apply_alg(&alg);
            }
        }
        if app.cube == start {
            view.cursor = 0;
        }
    }

    ui.vertical_centered(|ui| {
        ui.label(RichText::new(app.t(lesson.title)).size(24.0).strong());
    });

    // Reserve REAL space for the controls: karaoke row + step text (wraps
    // to two lines on phones) + two button rows + the 12px item spacing
    // between all of them + breathing room above the browser's bottom
    // bar. 170 was measured on desktop and cut the buttons in half on
    // mobile.
    let controls_height = 300.0;
    let cube_size = Vec2::new(
        ui.available_width(),
        (ui.available_height() - controls_height).max(120.0),
    );
    // The lesson cube is a SANDBOX: tap a sticker to select its layer and
    // turn it with the arrows — exploring is how kids learn. Replay
    // restores the step.
    let highlight = app
        .selected_face
        .map(|f| cube_core::Move::Face(f, cube_core::Turns::Cw));
    let response = CubeView {
        cube: &app.cube,
        animator: &app.animator,
        orbit: &mut app.orbit,
        highlight,
        dim_others: 1.0,
        color_override: None,
    }
    .show(ui, cube_size);
    super::play::handle_tap_select(app, &response);

    ui.vertical_centered(|ui| {
        karaoke_row(app, ui, view, step);
        ui.label(RichText::new(app.t(step.text)).size(22.0));
        ui.add_space(6.0);
        // ONE row: prev-step | Play/Restart | next-step. Play starts the
        // demo (1s breath); once started it becomes Restart. Switching
        // lesson/algorithm is the top-left back arrow's job — no other
        // buttons. While a sticker is selected, the row shows the
        // sandbox turn arrows instead.
        ui.horizontal(|ui| {
            let spacing = ui.spacing().item_spacing.x;
            if app.selected_face.is_some() {
                let unit = ((ui.available_width() - spacing * 6.0) / 5.0).clamp(40.0, 84.0);
                ui.add_space((ui.available_width() - 5.0 * (unit + spacing)).max(0.0) / 2.0);
                super::play::turn_arrows_sized(app, ui, Vec2::new(unit, 60.0));
            } else {
                let unit = ((ui.available_width() - spacing * 4.0) / 3.4).clamp(56.0, 96.0);
                let size = Vec2::new(unit, 60.0);
                let play_size = Vec2::new(unit * 1.4, 60.0);
                ui.add_space(
                    (ui.available_width() - 2.0 * (unit + spacing) - (play_size.x + spacing))
                        .max(0.0)
                        / 2.0,
                );
                let gray = Color32::from_gray(70);
                let first = view.step == 0;
                let last = view.step + 1 >= lesson.steps.len();
                let dim_gray = Color32::from_gray(45);
                if icons::big_icon_button(
                    ui,
                    size,
                    if first { dim_gray } else { gray },
                    "",
                    icons::draw_back_arrow,
                )
                .clicked()
                    && !first
                {
                    view.step -= 1;
                    view.pending = true;
                    view.playing = false;
                }
                let started = view.playing || view.cursor > 0;
                if started {
                    // Restart = back to the step's start, showing Play
                    // again (no auto-replay).
                    if icons::big_icon_button(
                        ui,
                        play_size,
                        Color32::from_rgb(0x2A, 0x5C, 0xC2),
                        "",
                        icons::draw_reset,
                    )
                    .clicked()
                    {
                        view.pending = true;
                        view.playing = false;
                    }
                } else if icons::big_icon_button(
                    ui,
                    play_size,
                    Color32::from_rgb(0x1E, 0x88, 0x50),
                    "",
                    icons::draw_play,
                )
                .clicked()
                {
                    view.playing = true;
                    view.play_at = now + 1.0;
                }
                if icons::big_icon_button(
                    ui,
                    size,
                    if last { Color32::from_rgb(0x1E, 0x88, 0x50) } else { gray },
                    "",
                    if last { icons::draw_check } else { icons::draw_next_arrow },
                )
                .clicked()
                {
                    if last {
                        *next = Some(Screen::Train(TrainScreen::Picker {
                            tab: PickerTab::Intro,
                        }));
                    } else {
                        view.step += 1;
                        view.pending = true;
                        view.playing = false;
                    }
                }
            }
        });
        ui.add_space(crate::app::BOTTOM_INSET);
    });
}

/// One tappable case card: diagram + name + trained star + best time.
/// Returns true when the card body is tapped.
fn case_card(app: &mut RubiksApp, ui: &mut Ui, case_idx: u16) -> bool {
    let def = app.library.rec.case(case_idx);
    let name = def.name.clone();
    let recognition = def.recognition;
    let size = Vec2::new(132.0, 168.0);
    let (rect, response) = ui.allocate_exact_size(size, Sense::click());
    let p = ui.painter();
    p.rect_filled(rect, 12.0, Color32::from_gray(38));
    if response.hovered() {
        p.rect_stroke(rect, 12.0, Stroke::new(2.0, Color32::WHITE), StrokeKind::Inside);
    }

    let diagram_rect = Rect::from_min_size(
        rect.min + Vec2::new((size.x - 96.0) / 2.0, 10.0),
        Vec2::splat(96.0),
    );
    match recognition {
        RecogKind::Oll | RecogKind::Pll => {
            let state = app.library.rec.canonical_state(case_idx);
            case_diagram::draw_ll_diagram(
                ui,
                diagram_rect,
                &state,
                matches!(recognition, RecogKind::Oll),
            );
        }
        _ => {
            // F2L/LBL: mini scrambled-face icon placeholder.
            icons::draw_mini_cube(p, diagram_rect.shrink(14.0), icons::scrambled_face());
        }
    }

    p.text(
        egui::Pos2::new(rect.center().x, rect.bottom() - 40.0),
        egui::Align2::CENTER_CENTER,
        &name,
        egui::FontId::proportional(14.0),
        Color32::WHITE,
    );

    // Trained star (own hit area).
    let star_rect = Rect::from_min_size(rect.min + Vec2::new(6.0, 6.0), Vec2::splat(24.0));
    let star_resp = ui.interact(star_rect, ui.id().with(("star", case_idx)), Sense::click());
    let trained = app.trained.contains(&case_idx);
    draw_star(
        p,
        star_rect,
        if trained {
            Color32::from_rgb(0xFF, 0xD5, 0x00)
        } else {
            Color32::from_gray(90)
        },
    );
    if star_resp.clicked() {
        app.toggle_trained(case_idx);
    }

    // Best time badge.
    if let Some(best) = best_ms(app, case_idx) {
        p.text(
            egui::Pos2::new(rect.center().x, rect.bottom() - 16.0),
            egui::Align2::CENTER_CENTER,
            format_ms(best),
            egui::FontId::proportional(15.0),
            Color32::from_rgb(0x4C, 0xD9, 0x64),
        );
    }

    response.clicked() && !star_resp.clicked()
}

fn best_ms(app: &RubiksApp, case_idx: u16) -> Option<u64> {
    app.attempts
        .get(&case_idx)?
        .iter()
        .filter(|a| a.success)
        .map(|a| a.ms)
        .min()
}

pub fn format_ms(ms: u64) -> String {
    format!("{}.{:02}", ms / 1000, (ms % 1000) / 10)
}

fn draw_star(p: &egui::Painter, r: Rect, color: Color32) {
    let c = r.center();
    let outer = r.width() / 2.0;
    let inner = outer * 0.45;
    let mut points = Vec::with_capacity(10);
    for i in 0..10 {
        let radius = if i % 2 == 0 { outer } else { inner };
        let a = -std::f32::consts::FRAC_PI_2 + i as f32 * std::f32::consts::PI / 5.0;
        points.push(egui::Pos2::new(c.x + radius * a.cos(), c.y + radius * a.sin()));
    }
    p.add(egui::Shape::convex_polygon(points, color, Stroke::NONE));
}

fn session_ui(app: &mut RubiksApp, ui: &mut Ui, session: &mut SessionState, next: &mut Option<Screen>) {
    let now = ui.input(|i| i.time);
    let def = app.library.rec.case(session.case_idx);
    let set = def.set;
    let name = def.name.clone();
    let alg_text = def.alg.to_string();

    ui.vertical_centered(|ui| {
        ui.label(RichText::new(name).size(26.0).strong());
    });

    // 3D cube shows the case; leave room for the control zone.
    let controls_height = 210.0;
    let cube_size = Vec2::new(
        ui.available_width(),
        (ui.available_height() - controls_height).max(120.0),
    );
    CubeView {
        cube: &app.cube,
        animator: &app.animator,
        orbit: &mut app.orbit,
        highlight: None,
        dim_others: 1.0,
        color_override: None,
    }
    .show(ui, cube_size);

    ui.vertical_centered(|ui| {
        match &mut session.phase {
        Phase::Ready => {
            if big_tap_zone(ui, Color32::from_rgb(0x1E, 0x88, 0x50), "GO", 56.0) {
                session.phase = Phase::Timing { start: now };
            }
            ui.horizontal(|ui| {
                ui.add_space((ui.available_width() - 220.0).max(0.0) / 2.0);
                if icons::big_icon_button(
                    ui,
                    Vec2::new(104.0, 56.0),
                    Color32::from_gray(60),
                    app.t(TextKey::ShowSolution),
                    icons::draw_play,
                )
                .clicked()
                    && app.animator.is_idle()
                {
                    // If the cube drifted (exploring, or a previous
                    // Show me already ran), silently doing nothing feels
                    // broken — reset to the case's canonical state and
                    // demonstrate from there. Cases WITHOUT recognition
                    // (the beginner lbl steps) always take that path:
                    // canonical state + the algorithm as written.
                    let mut m = app.library.rec.recognize_case(&app.cube, session.case_idx);
                    if m.is_none() {
                        app.cube = app.library.rec.canonical_state(session.case_idx);
                        app.selected_face = None;
                        m = app
                            .library
                            .rec
                            .recognize_case(&app.cube, session.case_idx)
                            .or(Some(cube_core::Match {
                                case_idx: session.case_idx,
                                pre_auf: 0,
                                y_frame: 0,
                            }));
                    }
                    if let Some(m) = m {
                        let exec = app.library.rec.execution_alg(m);
                        app.animator.enqueue_all(&exec.0);
                    }
                }
                if icons::big_icon_button(
                    ui,
                    Vec2::new(104.0, 56.0),
                    Color32::from_rgb(0x8E, 0x36, 0xB8),
                    app.t(TextKey::Scramble),
                    icons::draw_shuffle,
                )
                .clicked()
                {
                    new_case_state(app, session.case_idx);
                }
            });
            ui.label(RichText::new(alg_text).size(16.0).weak());
        }
        Phase::Timing { start } => {
            let elapsed = ((now - *start) * 1000.0) as u64;
            ui.label(
                RichText::new(format_ms(elapsed))
                    .size(64.0)
                    .strong()
                    .color(Color32::WHITE),
            );
            ui.ctx().request_repaint();
            if big_tap_zone(ui, Color32::from_rgb(0xC7, 0x2B, 0x2B), "STOP", 42.0) {
                session.phase = Phase::Result {
                    ms: elapsed,
                    success: None,
                };
            }
        }
        Phase::Result { ms, success } => {
            ui.label(RichText::new(format_ms(*ms)).size(52.0).strong());
            if success.is_none() {
                // Big visual verdict buttons: green check / red cross.
                ui.horizontal(|ui| {
                    ui.add_space((ui.available_width() - 230.0).max(0.0) / 2.0);
                    if icons::big_icon_button(
                        ui,
                        Vec2::new(110.0, 72.0),
                        Color32::from_rgb(0x1E, 0x88, 0x50),
                        "",
                        icons::draw_check,
                    )
                    .clicked()
                    {
                        *success = Some(true);
                        record(app, session.case_idx, *ms, true);
                    }
                    if icons::big_icon_button(
                        ui,
                        Vec2::new(110.0, 72.0),
                        Color32::from_rgb(0xC7, 0x2B, 0x2B),
                        "",
                        icons::draw_cross,
                    )
                    .clicked()
                    {
                        *success = Some(false);
                        record(app, session.case_idx, *ms, false);
                    }
                });
            } else {
                stats_row(app, ui, session.case_idx);
                if big_tap_zone(ui, Color32::from_rgb(0x2A, 0x5C, 0xC2), "GO", 42.0) {
                    new_case_state(app, session.case_idx);
                    session.phase = Phase::Ready;
                }
            }
        }
        }
        ui.add_space(crate::app::BOTTOM_INSET);
    });

    let _ = (set, next);
}

/// Delegates to the shared karaoke widget with this step's demo.
fn karaoke_row(app: &RubiksApp, ui: &mut Ui, view: &LessonView, step: &crate::lessons::Step) {
    let Some(demo) = step.demo else { return };
    let Ok(alg) = cube_core::Alg::parse(demo) else { return };
    // `cursor` counts ENQUEUED moves: highlighting cursor-1 while the
    // animator runs marks the move AS IT STARTS (not after it lands).
    crate::widgets::playback::karaoke_row(ui, &alg.0, view.cursor, !app.animator.is_idle());
}

fn new_case_state(app: &mut RubiksApp, case_idx: u16) {
    app.animator.clear();
    let mut rng = cube_core::SplitMix64::new(app.rng.next_u64());
    app.cube = app.library.rec.setup_state(case_idx, &mut rng);
}

fn record(app: &mut RubiksApp, case_idx: u16, ms: u64, success: bool) {
    app.record_attempt(case_idx, ms, success);
}

fn stats_row(app: &RubiksApp, ui: &mut Ui, case_idx: u16) {
    let attempts = app.attempts.get(&case_idx).cloned().unwrap_or_default();
    let successes: Vec<u64> = attempts.iter().filter(|a| a.success).map(|a| a.ms).collect();
    let best = successes.iter().min().copied();
    let last5: Vec<u64> = successes.iter().rev().take(5).copied().collect();
    let avg5 = (!last5.is_empty()).then(|| last5.iter().sum::<u64>() / last5.len() as u64);
    ui.horizontal(|ui| {
        ui.add_space((ui.available_width() - 320.0).max(0.0) / 2.0);
        stat_tile(ui, app.t(TextKey::Best), best.map(format_ms), Color32::from_rgb(0x4C, 0xD9, 0x64));
        stat_tile(ui, app.t(TextKey::Average), avg5.map(format_ms), Color32::from_rgb(0x58, 0xB6, 0xFF));
        stat_tile(
            ui,
            app.t(TextKey::Attempts),
            Some(attempts.len().to_string()),
            Color32::from_gray(200),
        );
    });
    sparkline(ui, &successes);
}

fn stat_tile(ui: &mut Ui, label: &str, value: Option<String>, color: Color32) {
    ui.vertical(|ui| {
        ui.label(RichText::new(label).size(13.0).weak());
        ui.label(
            RichText::new(value.unwrap_or_else(|| "–".into()))
                .size(22.0)
                .color(color),
        );
    });
}

/// Tiny painter-drawn time series of successful attempts (newest right).
fn sparkline(ui: &mut Ui, times: &[u64]) {
    if times.len() < 2 {
        return;
    }
    let (rect, _) = ui.allocate_exact_size(Vec2::new(220.0, 40.0), Sense::hover());
    let max = *times.iter().max().unwrap() as f32;
    let min = *times.iter().min().unwrap() as f32;
    let span = (max - min).max(1.0);
    let n = times.len();
    let pts: Vec<egui::Pos2> = times
        .iter()
        .enumerate()
        .map(|(i, &t)| {
            egui::Pos2::new(
                rect.left() + rect.width() * i as f32 / (n - 1) as f32,
                rect.bottom() - rect.height() * (1.0 - (t as f32 - min) / span) * 0.9 - 2.0,
            )
        })
        .collect();
    // Note: y maps larger times lower via inverted factor below.
    let p = ui.painter();
    for w in pts.windows(2) {
        p.line_segment([w[0], w[1]], Stroke::new(2.0, Color32::from_rgb(0x58, 0xB6, 0xFF)));
    }
}

/// A full-width tappable zone with huge text — the main interaction while
/// holding a physical cube (easy to hit without looking).
fn big_tap_zone(ui: &mut Ui, color: Color32, text: &str, font: f32) -> bool {
    let size = Vec2::new((ui.available_width() * 0.8).min(420.0), font + 26.0);
    let (rect, response) = ui.allocate_exact_size(size, Sense::click());
    let p = ui.painter();
    p.rect_filled(rect, 16.0, color);
    p.text(
        rect.center(),
        egui::Align2::CENTER_CENTER,
        text,
        egui::FontId::proportional(font),
        Color32::WHITE,
    );
    response.clicked()
}
