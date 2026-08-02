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
    /// The step's demo has completed at least once: unlocks Practice.
    pub watched: bool,
    /// Playback holds until this time: a 1s breath at start/restart so
    /// the eye finds the cube before colors start moving.
    pub play_at: f64,
}

pub struct SessionState {
    pub case_idx: u16,
    pub phase: Phase,
    /// The current example's start state: the recipe's live progress is
    /// derived by matching the cube against its prefix states.
    pub example: cube_core::FaceletCube,
    /// Set when Practice was launched from a lesson step: the top-left
    /// back arrow returns THERE, not to the picker.
    pub from_lesson: Option<(usize, usize)>,
}

pub enum Phase {
    /// See the algorithm: a steppable demo player (play/pause +
    /// prev/next). `origin` is the state the demo starts from (canonical
    /// from the picker; the current example from Show me), `exec` the
    /// algorithm to demonstrate, `cursor` how many moves are applied.
    Watch {
        origin: cube_core::FaceletCube,
        exec: cube_core::Alg,
        cursor: usize,
        playing: bool,
    },
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
                match session.from_lesson {
                    Some((lesson, step)) => Screen::Train(TrainScreen::Lesson(LessonView {
                        lesson,
                        step,
                        pending: true,
                        demo_len: 0,
                        cursor: 0,
                        playing: false,
                        play_at: 0.0,
                        watched: true, // they came from Practice: keep it unlocked
                    })),
                    None => Screen::Train(TrainScreen::Picker {
                        tab: PickerTab::Set(app.library.rec.case(session.case_idx).set),
                    }),
                }
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

/// Responsive card width: 3..=6 columns depending on screen width
/// (phones get 3, tablets/desktop more), each at least ~104 px.
fn card_width(ui: &Ui) -> f32 {
    let spacing = ui.spacing().item_spacing.x;
    let avail = ui.available_width();
    let cols = (((avail + spacing) / (116.0 + spacing)).floor()).clamp(3.0, 6.0);
    ((avail - (cols + 1.0) * spacing) / cols).clamp(96.0, 150.0)
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
        // Five tabs overflow narrow phones: horizontally draggable.
        ScrollArea::horizontal().show(ui, |ui| {
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
    });
    ui.add_space(8.0);

    match *tab {
        PickerTab::Intro => intro_picker(app, ui, next),
        PickerTab::Set(set) => {
            let cases = app.library.cases_in_set(set);
            ScrollArea::vertical().show(ui, |ui| {
                ui.horizontal_wrapped(|ui| {
                    let w = card_width(ui);
                    for case_idx in cases {
                        if case_card(app, ui, case_idx, w) {
                            // See it first: canonical state + demo player.
                            app.animator.clear();
                            app.selected_face = None;
                            let origin = app.library.rec.canonical_state(case_idx);
                            app.cube = origin;
                            *next = Some(Screen::Train(TrainScreen::Session(SessionState {
                                case_idx,
                                phase: Phase::Watch {
                                    origin,
                                    exec: demo_exec(app, case_idx, &origin),
                                    cursor: 0,
                                    playing: false,
                                },
                                from_lesson: None,
                                example: origin,
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
            let w = card_width(ui);
            for (i, lesson) in crate::lessons::LESSONS.iter().enumerate() {
                let sc = w / 132.0;
                let size = Vec2::new(w, 150.0 * sc);
                let (rect, response) = ui.allocate_exact_size(size, Sense::click());
                let p = ui.painter();
                p.rect_filled(rect, 12.0, Color32::from_gray(38));
                if response.hovered() {
                    p.rect_stroke(rect, 12.0, Stroke::new(2.0, Color32::WHITE), StrokeKind::Inside);
                }
                p.text(
                    egui::Pos2::new(rect.center().x, rect.top() + 52.0 * sc),
                    egui::Align2::CENTER_CENTER,
                    lesson.badge,
                    egui::FontId::proportional(56.0 * sc),
                    Color32::from_rgb(0x4C, 0xD9, 0x64),
                );
                p.text(
                    egui::Pos2::new(rect.center().x, rect.bottom() - 26.0 * sc),
                    egui::Align2::CENTER_CENTER,
                    app.t(lesson.title),
                    egui::FontId::proportional((15.0 * sc).max(11.0)),
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
                        watched: false,
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
        view.watched = true;
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

    // Reserve REAL space for the controls: karaoke (which WRAPS on long
    // demos), step text, Play row, prev/next row, spacing, bottom air —
    // plus the Practice button once unlocked. Underestimating pushes
    // buttons off-screen.
    let practice_unlocked = step.practice.is_some() && view.watched;
    let karaoke_lines = ((demo_moves.len() as f32 * 34.0)
        / ui.available_width().max(1.0))
    .ceil()
    .max(1.0);
    let controls_height = 249.0
        + karaoke_lines * 26.0
        + if practice_unlocked { 70.0 } else { 0.0 };
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
        hint: None,
    }
    .show(ui, cube_size);
    super::play::handle_tap_select(app, &response);

    ui.vertical_centered(|ui| {
        karaoke_row(app, ui, view, step);
        ui.label(RichText::new(app.t(step.text)).size(22.0));
        ui.add_space(6.0);
        // Play/Restart centered (or the sandbox turn arrows while a
        // sticker is selected); Practice under it once unlocked; and at
        // the bottom a FULL-WIDTH prev/next pair shared with every other
        // stepping screen (left half back, right half forward).
        ui.horizontal(|ui| {
            let spacing = ui.spacing().item_spacing.x;
            if app.selected_face.is_some() {
                let unit = ((ui.available_width() - spacing * 6.0) / 5.0).clamp(40.0, 84.0);
                ui.add_space((ui.available_width() - 5.0 * (unit + spacing)).max(0.0) / 2.0);
                super::play::turn_arrows_sized(app, ui, Vec2::new(unit, 60.0));
            } else {
                // Big Play/Pause; a small Restart appears beside it once
                // the demo has started.
                let spacing = ui.spacing().item_spacing.x;
                let play_size = Vec2::new(132.0, 60.0);
                let small = Vec2::new(60.0, 60.0);
                let started = view.cursor > 0 || view.playing;
                let row_w = if started {
                    play_size.x + spacing + small.x
                } else {
                    play_size.x
                };
                ui.add_space((ui.available_width() - row_w).max(0.0) / 2.0);
                if started
                    && icons::big_icon_button(
                        ui,
                        small,
                        Color32::from_rgb(0x2A, 0x5C, 0xC2),
                        "",
                        icons::draw_reset,
                    )
                    .clicked()
                {
                    view.pending = true;
                    view.playing = false;
                }
                let at_end = view.cursor >= demo_moves.len();
                if icons::big_icon_button(
                    ui,
                    play_size,
                    Color32::from_rgb(0x1E, 0x88, 0x50),
                    "",
                    if view.playing { icons::draw_pause } else { icons::draw_play },
                )
                .clicked()
                {
                    if view.playing {
                        view.playing = false; // PAUSE (current move finishes)
                    } else if at_end && !demo_moves.is_empty() {
                        // Play at the end = replay with the 1s breath.
                        view.pending = true;
                        view.playing = true;
                    } else if view.cursor > 0 {
                        view.playing = true; // resume instantly
                        view.play_at = now;
                    } else {
                        view.playing = true; // fresh start: 1s breath
                        view.play_at = now + 1.0;
                    }
                }
            }
        });
        // Practice unlocks after the demo has been WATCHED: repeated
        // drilling of this algorithm with fresh random examples.
        if practice_unlocked {
            if let Some(case_id) = step.practice {
                if let Some(case_idx) = app.library.rec.find_by_id(case_id) {
                    if icons::big_icon_button(
                        ui,
                        Vec2::new(190.0, 60.0),
                        Color32::from_rgb(0x8E, 0x36, 0xB8),
                        app.t(TextKey::Practice),
                        icons::draw_dumbbell,
                    )
                    .clicked()
                    {
                        new_case_state(app, case_idx);
                        *next = Some(Screen::Train(TrainScreen::Session(SessionState {
                            case_idx,
                            phase: Phase::Ready,
                            from_lesson: Some((view.lesson, view.step)),
                            example: app.cube,
                        })));
                    }
                }
            }
        }
        let first = view.step == 0;
        let last = view.step + 1 >= lesson.steps.len();
        let (prev, nxt) = crate::widgets::playback::prev_next_row(ui, !first, last);
        if prev {
            view.step -= 1;
            view.pending = true;
            view.playing = false;
            view.watched = false;
        }
        if nxt {
            if last {
                *next = Some(Screen::Train(TrainScreen::Picker {
                    tab: PickerTab::Intro,
                }));
            } else {
                view.step += 1;
                view.pending = true;
                view.playing = false;
                view.watched = false;
            }
        }
        ui.add_space(crate::app::BOTTOM_INSET);
    });
}

/// One tappable case card: diagram + name + trained star + best time.
/// Returns true when the card body is tapped.
fn case_card(app: &mut RubiksApp, ui: &mut Ui, case_idx: u16, w: f32) -> bool {
    let def = app.library.rec.case(case_idx);
    let name = def.name.clone();
    let recognition = def.recognition;
    let sc = w / 132.0;
    let size = Vec2::new(w, 168.0 * sc);
    let (rect, response) = ui.allocate_exact_size(size, Sense::click());
    let p = ui.painter();
    p.rect_filled(rect, 12.0, Color32::from_gray(38));
    if response.hovered() {
        p.rect_stroke(rect, 12.0, Stroke::new(2.0, Color32::WHITE), StrokeKind::Inside);
    }

    let diagram_rect = Rect::from_min_size(
        rect.min + Vec2::new((size.x - 96.0 * sc) / 2.0, 10.0 * sc),
        Vec2::splat(96.0 * sc),
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
        egui::Pos2::new(rect.center().x, rect.bottom() - 40.0 * sc),
        egui::Align2::CENTER_CENTER,
        &name,
        egui::FontId::proportional((14.0 * sc).max(11.0)),
        Color32::WHITE,
    );

    // Trained star (own hit area).
    let star_rect =
        Rect::from_min_size(rect.min + Vec2::new(6.0, 6.0), Vec2::splat((24.0 * sc).max(20.0)));
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
            egui::Pos2::new(rect.center().x, rect.bottom() - 16.0 * sc),
            egui::Align2::CENTER_CENTER,
            format_ms(best),
            egui::FontId::proportional((15.0 * sc).max(11.0)),
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

    let (recipe_example, recipe_case) = (session.example, session.case_idx);
    // The letter the learner is on: its layer gets a yellow wash so the
    // notation and the pieces it takes are visibly the same thing.
    let recipe_hint = if matches!(app.hints.mode, crate::app::HintMode::Off) {
        None
    } else {
        recipe_next_move(app, recipe_example, recipe_case)
    };
    // 3D cube shows the case; the turn-arrow row's space is ALWAYS
    // reserved so the cube never jumps when arrows appear/disappear.
    let controls_height = 282.0;
    let cube_size = Vec2::new(
        ui.available_width(),
        (ui.available_height() - controls_height).max(120.0),
    );
    let response = CubeView {
        cube: &app.cube,
        animator: &app.animator,
        orbit: &mut app.orbit,
        // SANDBOX like the lesson cube: tap a sticker to select its
        // layer and turn it with the arrows — trying the algorithm ON
        // the cube is half the practice.
        highlight: app
            .selected_face
            .map(|f| cube_core::Move::Face(f, cube_core::Turns::Cw)),
        dim_others: 1.0,
        color_override: None,
        hint: recipe_hint,
    }
    .show(ui, cube_size);
    super::play::handle_tap_select(app, &response);

    ui.vertical_centered(|ui| {
        // Fixed-height arrow slot: filled while a sticker is selected,
        // empty otherwise — the layout below never shifts.
        ui.horizontal(|ui| {
            if app.selected_face.is_some() {
                let spacing = ui.spacing().item_spacing.x;
                let unit = ((ui.available_width() - spacing * 6.0) / 5.0).clamp(40.0, 84.0);
                ui.add_space((ui.available_width() - 5.0 * (unit + spacing)).max(0.0) / 2.0);
                super::play::turn_arrows_sized(app, ui, Vec2::new(unit, 60.0));
            } else {
                ui.allocate_exact_size(Vec2::new(1.0, 60.0), Sense::hover());
            }
        });
        match &mut session.phase {
        Phase::Watch {
            origin,
            exec,
            cursor,
            playing,
        } => {
            let total = exec.0.len();
            // Drip one move at a time while playing (pause/step exact).
            if *playing && *cursor < total && app.animator.is_idle() {
                app.animator.enqueue(exec.0[*cursor]);
                *cursor += 1;
            }
            if *playing && *cursor < total {
                ui.ctx().request_repaint();
            }
            if *playing && *cursor >= total && app.animator.is_idle() {
                *playing = false;
            }
            // Karaoke marks each move as it STARTS.
            crate::widgets::playback::karaoke_row(
                ui,
                &exec.0,
                *cursor,
                !app.animator.is_idle(),
            );
            let at_end = *cursor >= total;
            ui.horizontal(|ui| {
                let spacing = ui.spacing().item_spacing.x;
                let size = Vec2::new(64.0, 56.0);
                let play_size = Vec2::new(120.0, 56.0);
                ui.add_space(
                    (ui.available_width() - 2.0 * (size.x + spacing) - (play_size.x + spacing))
                        .max(0.0)
                        / 2.0,
                );
                let gray = Color32::from_gray(70);
                // Step back: undo the previous move (inverse, animated).
                if icons::big_icon_button(ui, size, gray, "", icons::draw_chevron_step_back)
                    .clicked()
                    && *cursor > 0
                    && app.animator.is_idle()
                {
                    *playing = false;
                    *cursor -= 1;
                    app.animator.enqueue(exec.0[*cursor].inverse());
                }
                // Play / pause (play at the end = replay from origin).
                if icons::big_icon_button(
                    ui,
                    play_size,
                    Color32::from_rgb(0x1E, 0x88, 0x50),
                    "",
                    if *playing { icons::draw_pause } else { icons::draw_play },
                )
                .clicked()
                {
                    if *playing {
                        *playing = false;
                    } else if at_end && app.animator.is_idle() {
                        app.cube = *origin;
                        *cursor = 0;
                        *playing = true;
                    } else {
                        *playing = true;
                    }
                }
                // Step forward: one move, animated.
                if icons::big_icon_button(ui, size, gray, "", icons::draw_chevron_step_fwd)
                    .clicked()
                    && *cursor < total
                    && app.animator.is_idle()
                {
                    *playing = false;
                    app.animator.enqueue(exec.0[*cursor]);
                    *cursor += 1;
                }
            });
            // Practice: into the drill loop with a fresh random example.
            if icons::big_icon_button(
                ui,
                Vec2::new(190.0, 60.0),
                Color32::from_rgb(0x8E, 0x36, 0xB8),
                app.t(TextKey::Practice),
                icons::draw_dumbbell,
            )
            .clicked()
            {
                new_case_state(app, session.case_idx);
                session.example = app.cube;
                session.phase = Phase::Ready;
            }
            ui.label(RichText::new(alg_text.clone()).size(16.0).weak());
        }
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
                    // Open the steppable demo player (pause + prev/next)
                    // on THIS example. If the cube drifted (exploring, a
                    // previous demo) or the case has no recognition (the
                    // beginner lbl steps), reset to the canonical state
                    // and demonstrate from there.
                    if app
                        .library
                        .rec
                        .recognize_case(&app.cube, session.case_idx)
                        .is_none()
                    {
                        app.cube = app.library.rec.canonical_state(session.case_idx);
                        app.selected_face = None;
                    }
                    app.animator.clear();
                    let origin = app.cube;
                    session.phase = Phase::Watch {
                        exec: demo_exec(app, session.case_idx, &origin),
                        origin,
                        cursor: 0,
                        playing: true,
                    };
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
                    session.example = app.cube;
                }
            });
            live_recipe(app, ui, recipe_example, recipe_case);
        }
        Phase::Timing { start } => {
            live_recipe(app, ui, recipe_example, recipe_case);
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
                    session.example = app.cube;
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

/// The execution alg demonstrating `case_idx` from `state` (recognized
/// AUF included; falls back to the algorithm as written).
fn demo_exec(
    app: &RubiksApp,
    case_idx: u16,
    state: &cube_core::FaceletCube,
) -> cube_core::Alg {
    let m = app
        .library
        .rec
        .recognize_case(state, case_idx)
        .unwrap_or(cube_core::Match {
            case_idx,
            pre_auf: 0,
            y_frame: 0,
        });
    app.library.rec.execution_alg(m)
}

/// The move the learner is on in the recipe (the first one not yet
/// performed), for the yellow piece highlight.
fn recipe_next_move(
    app: &RubiksApp,
    example: cube_core::FaceletCube,
    case_idx: u16,
) -> Option<cube_core::Move> {
    let exec = demo_exec(app, case_idx, &example);
    let mut probe = example;
    for (k, &m) in exec.0.iter().enumerate() {
        if probe == app.cube {
            return exec.0.get(k).copied();
        }
        probe.apply(m);
        let _ = k;
    }
    None
}

/// The recipe, LIVE: a big karaoke row where every move lights green as
/// the user actually performs it on the cube (state-prefix matching, so
/// undoing a wrong turn naturally rolls the marker back). Shown when
/// hints are on; practice mode shows progress dots without letters.
fn live_recipe(app: &RubiksApp, ui: &mut Ui, example: cube_core::FaceletCube, case_idx: u16) {
    if matches!(app.hints.mode, crate::app::HintMode::Off) {
        return;
    }
    let exec = demo_exec(app, case_idx, &example);
    if exec.0.is_empty() {
        return;
    }
    // Largest prefix of the recipe the cube has actually reached.
    let mut done = 0;
    let mut probe = example;
    for (k, &m) in exec.0.iter().enumerate() {
        if probe == app.cube {
            done = k;
            break;
        }
        probe.apply(m);
        done = k + 1;
    }
    if probe != app.cube && done == exec.0.len() {
        // Cube is off the recipe path entirely: mark nothing.
        done = 0;
    }
    let practice = matches!(app.hints.mode, crate::app::HintMode::Practice);
    let hidden: Vec<bool> = vec![practice; exec.0.len()];
    crate::widgets::playback::karaoke_row_masked(
        ui,
        &exec.0,
        done,
        false,
        practice.then_some(hidden.as_slice()),
    );
}

fn new_case_state(app: &mut RubiksApp, case_idx: u16) {
    app.animator.clear();
    app.selected_face = None;
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
