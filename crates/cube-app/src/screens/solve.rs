//! Solve mode (M3): enter a cube on the 2D net (camera scan lands in M4),
//! validate it with friendly visual feedback, then follow a step-by-step
//! 3D guide that solves it in ≤ 21 moves.

use crate::app::{RubiksApp, Screen, TableState};
use crate::i18n::TextKey;
use crate::widgets::{cube_view::CubeView, icons, net2d};
use cube_core::{Alg, FaceletCube};
use cube_solver::ValidationError;
use egui::{Color32, RichText, Sense, Ui, Vec2};

pub enum SolveScreen {
    Input {
        draft: FaceletCube,
        error: Option<ValidationError>,
    },
    Guide(GuideState),
}

pub struct GuideState {
    pub solution: Alg,
    /// Per-move label: Some(name) while inside a trained-algorithm segment
    /// of a guided solution ("T-Perm" chip in the UI).
    pub labels: Vec<Option<String>>,
    /// Moves of `solution` already applied to `app.cube`.
    pub cursor: usize,
    pub playing: bool,
    /// The state the guide started from: the top-left arrow goes back to
    /// the review net with this, not to the menu.
    pub origin: FaceletCube,
}

impl GuideState {
    pub fn plain(solution: Alg, origin: FaceletCube) -> GuideState {
        let labels = vec![None; solution.0.len()];
        GuideState {
            solution,
            labels,
            cursor: 0,
            playing: false,
            origin,
        }
    }

    /// Build from the hint engine's output: use the guided (trained-alg
    /// weaving) solution when it exists, else the plain one.
    pub fn from_output(
        out: cube_solver::SolveOutput,
        rec: &cube_core::Recognizer,
        origin: FaceletCube,
    ) -> GuideState {
        match out.guided {
            None => GuideState::plain(out.base, origin),
            Some(guided) => {
                let mut moves = Vec::new();
                let mut labels = Vec::new();
                for seg in &guided.segments {
                    let label = match seg {
                        cube_solver::Segment::Raw(_) => None,
                        cube_solver::Segment::Trained { case_idx, .. } => {
                            Some(rec.case(*case_idx).name.clone())
                        }
                    };
                    for &m in &seg.alg().0 {
                        moves.push(m);
                        labels.push(label.clone());
                    }
                }
                GuideState {
                    solution: Alg::new(moves),
                    labels,
                    cursor: 0,
                    playing: false,
                    origin,
                }
            }
        }
    }
}

impl SolveScreen {
    pub fn new_input(current: FaceletCube) -> Self {
        SolveScreen::Input {
            draft: current.normalize_orientation(),
            error: None,
        }
    }
}

pub fn show(app: &mut RubiksApp, ui: &mut Ui) {
    super::play::top_bar(app, ui);
    // Take the screen state out so we can borrow app mutably alongside.
    let Screen::Solve(mut screen) = std::mem::replace(&mut app.screen, Screen::Menu) else {
        return;
    };
    let mut next: Option<Screen> = None;

    match &mut screen {
        SolveScreen::Input { draft, error } => {
            ui.vertical_centered(|ui| {
                if net2d::net_editor(ui, draft, true) {
                    *error = None;
                }
                ui.add_space(8.0);

                match &app.table {
                    TableState::Loading => {
                        ui.spinner();
                        ui.label(app.t(TextKey::TableLoading));
                    }
                    TableState::Failed(e) => {
                        ui.colored_label(Color32::LIGHT_RED, e.clone());
                    }
                    TableState::Ready => {}
                }

                ui.horizontal(|ui| {
                    ui.add_space((ui.available_width() - 3.0 * 108.0).max(0.0) / 2.0);
                    let size = Vec2::new(96.0, 76.0);
                    if icons::big_icon_button(
                        ui,
                        size,
                        Color32::from_rgb(0x8E, 0x36, 0xB8),
                        app.t(TextKey::Scramble),
                        icons::draw_shuffle,
                    )
                    .clicked()
                    {
                        *draft = cube_solver::random_state();
                        *error = None;
                    }
                    let can_solve = matches!(app.table, TableState::Ready);
                    let solve_color = if can_solve {
                        Color32::from_rgb(0x1E, 0x88, 0x50)
                    } else {
                        Color32::from_gray(70)
                    };
                    if icons::big_icon_button(ui, size, solve_color, app.t(TextKey::MenuSolve), |p, r| {
                        icons::draw_check(p, r)
                    })
                    .clicked()
                        && can_solve
                    {
                        match cube_solver::validate(draft) {
                            Err(e) => *error = Some(e),
                            Ok(()) => match solve_cached(app, draft) {
                                Err(e) => {
                                    *error = Some(ValidationError::BadFaceletString);
                                    log::error!("solve failed: {e}");
                                }
                                Ok(solution) => {
                                    let origin = draft.normalize_orientation();
                                    let trained: Vec<u16> =
                                        app.trained.iter().copied().collect();
                                    let guide = if trained.is_empty() {
                                        GuideState::plain(solution, origin)
                                    } else {
                                        match cube_solver::solve_with_hints(
                                            draft,
                                            &trained,
                                            &app.library.rec,
                                        ) {
                                            Ok(out) => GuideState::from_output(
                                                out,
                                                &app.library.rec,
                                                origin,
                                            ),
                                            Err(_) => GuideState::plain(solution, origin),
                                        }
                                    };
                                    app.cube = origin;
                                    app.animator.clear();
                                    next =
                                        Some(Screen::Solve(SolveScreen::Guide(guide)));
                                }
                            },
                        }
                    }
                });

                if let Some(e) = error {
                    ui.add_space(4.0);
                    // Big red X + short text: visual first.
                    ui.colored_label(
                        Color32::from_rgb(0xE0, 0x5A, 0x5A),
                        RichText::new(format!("✘ {}", error_text(app, e))).size(22.0),
                    );
                }
            });
        }
        SolveScreen::Guide(guide) => {
            show_guide(app, ui, guide, &mut next);
        }
    }

    app.screen = next.unwrap_or(Screen::Solve(screen));
}

fn error_text(app: &RubiksApp, e: &ValidationError) -> String {
    match e {
        ValidationError::ColorCounts(counts) => {
            let mut parts = Vec::new();
            for (i, &c) in counts.iter().enumerate() {
                if c != 9 {
                    parts.push(format!("{}: {}", cube_core::Face::from_index(i).letter(), c));
                }
            }
            format!("{} ({})", app.t(TextKey::ErrColorCounts), parts.join(", "))
        }
        ValidationError::CentersNotDistinct => app.t(TextKey::ErrCenters).to_string(),
        ValidationError::ImpossiblePiece => app.t(TextKey::ErrImpossiblePiece).to_string(),
        ValidationError::Unsolvable => app.t(TextKey::ErrUnsolvable).to_string(),
        ValidationError::BadFaceletString => app.t(TextKey::ErrUnsolvable).to_string(),
    }
}

fn show_guide(app: &mut RubiksApp, ui: &mut Ui, guide: &mut GuideState, next: &mut Option<Screen>) {
    // Calm default tempo at the start of a guide: each move must be
    // readable on the first viewing (the chevrons adjust it after).
    if guide.cursor == 0 && app.animator.is_idle() && app.animator.secs_per_quarter < 0.5 {
        app.animator.secs_per_quarter = 0.5;
    }
    let done = guide.cursor >= guide.solution.0.len();

    // Auto-play: feed the next move once the animator is idle.
    // Auto-play removed: impossible to follow on a real cube. The guide
    // is stepped move by move with the big Next button.
    if false && !done && app.animator.is_idle() {
        app.animator.enqueue(guide.solution.0[guide.cursor]);
        guide.cursor += 1;
        ui.ctx().request_repaint();
    }

    // Reserve space honestly: the karaoke row wraps on narrow phones.
    let avail_w = ui.available_width();
    let karaoke_lines = ((guide.solution.0.len() as f32 * 34.0) / avail_w.max(1.0)).ceil();
    let controls_height = 150.0 + karaoke_lines * 26.0 + crate::app::BOTTOM_INSET;
    let cube_size = Vec2::new(
        avail_w,
        (ui.available_height() - controls_height).max(120.0),
    );
    let highlight = (!done && app.animator.is_idle()).then(|| guide.solution.0[guide.cursor]);
    CubeView {
        cube: &app.cube,
        animator: &app.animator,
        orbit: &mut app.orbit,
        highlight,
        dim_others: 0.35,
        color_override: None,
    }
    .show(ui, cube_size);

    ui.vertical_centered(|ui| {
        // Karaoke letters carry both the plan and the progress.
        let played = guide.cursor.saturating_sub(app.animator.pending());
        crate::widgets::playback::karaoke_row(
            ui,
            &guide.solution.0,
            played,
            !app.animator.is_idle(),
        );

        // Trained-algorithm chip: shows WHICH known algorithm this part of
        // the solution is ("you know this bit!").
        if let Some(Some(label)) = guide.labels.get(guide.cursor.min(guide.labels.len().saturating_sub(1))) {
            if !done {
                ui.colored_label(
                    Color32::from_rgb(0xFF, 0xD5, 0x00),
                    RichText::new(format!("★ {label}")).size(22.0).strong(),
                );
            }
        }

        // Current / next move in large type.
        if done && app.animator.is_idle() {
            ui.colored_label(
                Color32::from_rgb(0x4C, 0xD9, 0x64),
                RichText::new(format!("✔ {}", app.t(TextKey::GuideDone))).size(34.0),
            );
        } else if let Some(m) = highlight {
            ui.label(RichText::new(m.to_string()).size(40.0).strong());
        } else {
            ui.label(RichText::new(" ").size(40.0));
        }

        ui.horizontal(|ui| {
            // slower faster | BIG GREEN NEXT. No auto-play and no move-
            // undo button: two kinds of "back" confused everyone (the
            // top-left arrow is the only back, one level up).
            let spacing = ui.spacing().item_spacing.x;
            let total = ui.available_width() - spacing * 4.0;
            let unit = (total / 3.6).clamp(44.0, 96.0);
            let size = Vec2::new(unit, 64.0f32.min(unit * 0.8));
            let small = Vec2::new(unit * 0.8, size.y);
            let next_size = Vec2::new(unit * 1.6, size.y);
            ui.add_space(
                (ui.available_width() - 2.0 * (small.x + spacing) - (next_size.x + spacing))
                    .max(0.0)
                    / 2.0,
            );
            crate::widgets::playback::speed_buttons_sized(app, ui, small);
            if icons::big_icon_button(
                ui,
                next_size,
                Color32::from_rgb(0x1E, 0x88, 0x50),
                app.t(TextKey::Next),
                icons::draw_next_arrow,
            )
            .clicked()
                && !done
                && app.animator.is_idle()
            {
                app.animator.enqueue(guide.solution.0[guide.cursor]);
                guide.cursor += 1;
            }
        });
        ui.add_space(crate::app::BOTTOM_INSET);
    });

    if done && app.animator.is_idle() {
        // Offer going back to the menu with one tap on the checkmark row.
        let (rect, response) = ui.allocate_exact_size(Vec2::new(1.0, 1.0), Sense::hover());
        let _ = (rect, response);
    }
    let _ = next;
}


/// Solve with the persistent MPEdb solution cache: same physical cube
/// scanned again (or revisited) resolves instantly; misses are stored so
/// they are only ever computed once per device.
fn solve_cached(
    app: &RubiksApp,
    state: &FaceletCube,
) -> Result<Alg, cube_solver::SolveError> {
    // v2: solutions cached before the shallow-solve fix may be the long
    // two-phase answer for nearly-solved states - don't serve those.
    let key = format!("v2:{}", state.normalize_orientation().to_facelet_string());
    if let Some(store) = &app.store {
        if let Ok(Some(cached)) = store.cached_solution(&key) {
            if let Ok(alg) = Alg::parse(&cached) {
                return Ok(alg);
            }
        }
    }
    let t0 = ui_now_ms();
    let solution = cube_solver::solve(state)?;
    if let Some(store) = &app.store {
        let _ = store.put_solution(
            &key,
            &solution.to_string(),
            solution.len_htm() as i64,
            (ui_now_ms() - t0).max(0),
        );
        crate::persist::persist(store);
    }
    Ok(solution)
}

fn ui_now_ms() -> i64 {
    crate::persist::now_ms()
}
