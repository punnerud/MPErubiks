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
    /// When the solve finished (drives the confetti cannon).
    pub done_at: Option<f64>,
    /// Extra confetti bursts: every tap on the solved cube fires again.
    pub bursts: Vec<f64>,
    /// Parallel to `solution.0`: Some(k) = the move belongs to trained
    /// segment k's OWN algorithm moves (leading AUF excluded). Labels
    /// alone cannot separate adjacent segments with the same name.
    pub seg_id: Vec<Option<u16>>,
    /// Practice-stop mode, captured when the guide was built.
    pub practice: bool,
    /// Gated segments whose letters the user asked to see (needed help).
    pub revealed: std::collections::HashSet<u16>,
    /// «Min vei»: predicted total human time for this plan, ms.
    pub est_ms: Option<u32>,
}

impl GuideState {
    pub fn plain(solution: Alg, origin: FaceletCube) -> GuideState {
        let len = solution.0.len();
        let labels = vec![None; len];
        GuideState {
            solution,
            labels,
            cursor: 0,
            playing: false,
            origin,
            done_at: None,
            bursts: Vec::new(),
            seg_id: vec![None; len],
            practice: false,
            revealed: std::collections::HashSet::new(),
            est_ms: None,
        }
    }

    /// Build from a guided solution the caller already computed (the
    /// practice scramble in Play knows its own answer).
    pub fn from_guided(
        guided: cube_solver::GuidedSolution,
        rec: &cube_core::Recognizer,
        origin: FaceletCube,
        practice: bool,
    ) -> GuideState {
        GuideState::from_output(
            cube_solver::SolveOutput {
                base: Alg::new(Vec::new()),
                inline_hints: Vec::new(),
                guided: Some(guided),
            },
            rec,
            origin,
            practice,
        )
    }

    /// Build from a «Min vei» macro plan: every named segment carries its
    /// algorithm name (and gates under practice mode like any trained
    /// segment).
    pub fn from_my_way(
        plan: cube_solver::MyWayPlan,
        rec: &cube_core::Recognizer,
        origin: FaceletCube,
        practice: bool,
    ) -> GuideState {
        let mut moves = Vec::new();
        let mut labels = Vec::new();
        let mut seg_id = Vec::new();
        let mut k: u16 = 0;
        for seg in &plan.segments {
            let (label, gate) = match seg.case_idx {
                Some(idx) => {
                    k += 1;
                    (Some(rec.case(idx).name.clone()), Some(k - 1))
                }
                None => (None, None),
            };
            for (i, &m) in seg.alg.0.iter().enumerate() {
                moves.push(m);
                labels.push(label.clone());
                seg_id.push(if i < seg.auf_len as usize { None } else { gate });
            }
        }
        GuideState {
            solution: Alg::new(moves),
            labels,
            cursor: 0,
            playing: false,
            origin,
            done_at: None,
            bursts: Vec::new(),
            seg_id,
            practice,
            revealed: std::collections::HashSet::new(),
            est_ms: Some(plan.total_ms),
        }
    }

    /// Build from the hint engine's output: use the guided (trained-alg
    /// weaving) solution when it exists, else the plain one.
    pub fn from_output(
        out: cube_solver::SolveOutput,
        rec: &cube_core::Recognizer,
        origin: FaceletCube,
        practice: bool,
    ) -> GuideState {
        match out.guided {
            None => GuideState::plain(out.base, origin),
            Some(guided) => {
                let mut moves = Vec::new();
                let mut labels = Vec::new();
                let mut seg_id = Vec::new();
                let mut k: u16 = 0;
                for seg in &guided.segments {
                    let (label, gate) = match seg {
                        cube_solver::Segment::Raw(_) => (None, None),
                        cube_solver::Segment::Trained { case_idx, .. } => {
                            k += 1;
                            (Some(rec.case(*case_idx).name.clone()), Some(k - 1))
                        }
                    };
                    let auf_len = match seg {
                        cube_solver::Segment::Trained { auf_len, .. } => *auf_len as usize,
                        _ => 0,
                    };
                    for (i, &m) in seg.alg().0.iter().enumerate() {
                        moves.push(m);
                        labels.push(label.clone());
                        // The leading AUF is not part of the algorithm:
                        // it is shown/stepped normally, never gated.
                        seg_id.push(if i < auf_len { None } else { gate });
                    }
                }
                GuideState {
                    solution: Alg::new(moves),
                    labels,
                    cursor: 0,
                    playing: false,
                    origin,
                    done_at: None,
                    bursts: Vec::new(),
                    seg_id,
                    practice,
                    revealed: std::collections::HashSet::new(),
                    est_ms: None,
                }
            }
        }
    }

    /// Some(k) while practice-stop blocks stepping at trained segment k.
    /// Derived, never stored — no stale-flag bugs by construction.
    pub fn gate(&self) -> Option<u16> {
        if !self.practice {
            return None;
        }
        let k = (*self.seg_id.get(self.cursor)?)?;
        (!self.revealed.contains(&k)).then_some(k)
    }

    /// One past the last move of segment k, scanning from the cursor.
    pub fn seg_end(&self, k: u16) -> usize {
        let mut i = self.cursor;
        while self.seg_id.get(i).copied().flatten() == Some(k) {
            i += 1;
        }
        i
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
    let bar = super::play::top_bar_with_gear(ui);
    if matches!(bar, super::play::TopBarAction::Gear) {
        let prev = std::mem::replace(&mut app.screen, Screen::Menu);
        app.screen = Screen::Settings(crate::screens::settings::SettingsScreen {
            prev: Box::new(prev),
            focus: crate::screens::settings::SettingsFocus::General,
        });
        return;
    }
    // Take the screen state out so we can borrow app mutably alongside.
    let Screen::Solve(mut screen) = std::mem::replace(&mut app.screen, Screen::Menu) else {
        return;
    };
    let mut next: Option<Screen> = None;
    // Hierarchical back: guide -> review net (original state restored);
    // review net -> menu.
    if matches!(bar, super::play::TopBarAction::Back) {
        next = Some(match &screen {
            SolveScreen::Guide(guide) => {
                app.animator.clear();
                app.cube = guide.origin;
                Screen::Solve(SolveScreen::Input {
                    draft: guide.origin,
                    error: None,
                })
            }
            SolveScreen::Input { .. } => Screen::Menu,
        });
    }

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
                    ui.add_space((ui.available_width() - 4.0 * 108.0).max(0.0) / 2.0);
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
                                    // Ordered include list from settings,
                                    // filtered to still-trained cases.
                                    let include: Vec<u16> = app
                                        .hints
                                        .include
                                        .iter()
                                        .copied()
                                        .filter(|c| app.trained.contains(c))
                                        .collect();
                                    let off =
                                        matches!(app.hints.mode, crate::app::HintMode::Off);
                                    let guide = if off || include.is_empty() {
                                        GuideState::plain(solution, origin)
                                    } else {
                                        match cube_solver::solve_with_hints(
                                            draft,
                                            &include,
                                            &app.library.rec,
                                            app.hints.max_extra,
                                        ) {
                                            Ok(out) => GuideState::from_output(
                                                out,
                                                &app.library.rec,
                                                origin,
                                                matches!(
                                                    app.hints.mode,
                                                    crate::app::HintMode::Practice
                                                ),
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
                    // «Min vei»: the macro route through the user's OWN
                    // algorithms, with predicted human time.
                    if icons::big_icon_button(
                        ui,
                        size,
                        if can_solve {
                            Color32::from_rgb(0xC7, 0x8A, 0x08)
                        } else {
                            Color32::from_gray(70)
                        },
                        app.t(TextKey::MyWay),
                        icons::draw_star,
                    )
                    .clicked()
                        && can_solve
                    {
                        match cube_solver::validate(draft) {
                            Err(e) => *error = Some(e),
                            Ok(()) => {
                                let origin = draft.normalize_orientation();
                                let attempts = app.attempts.clone();
                                let cost = |case: u16| -> Option<u32> {
                                    let list = attempts.get(&case)?;
                                    let ok: Vec<u64> = list
                                        .iter()
                                        .filter(|a| a.success)
                                        .map(|a| a.ms)
                                        .collect();
                                    if ok.is_empty() {
                                        return None;
                                    }
                                    Some((ok.iter().sum::<u64>() / ok.len() as u64) as u32)
                                };
                                match cube_solver::my_way(&origin, &app.library.rec, &cost) {
                                    Some(plan) => {
                                        let practice = matches!(
                                            app.hints.mode,
                                            crate::app::HintMode::Practice
                                        );
                                        let guide = GuideState::from_my_way(
                                            plan,
                                            &app.library.rec,
                                            origin,
                                            practice,
                                        );
                                        app.cube = origin;
                                        app.animator.clear();
                                        next = Some(Screen::Solve(SolveScreen::Guide(guide)));
                                    }
                                    None => *error = Some(ValidationError::Unsolvable),
                                }
                            }
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


    // Reserve space honestly: the karaoke row wraps on narrow phones,
    // and the practice gate adds the prompt line.
    let avail_w = ui.available_width();
    let karaoke_lines = ((guide.solution.0.len() as f32 * 34.0) / avail_w.max(1.0)).ceil();
    let gate_extra = if guide.practice { 34.0 } else { 0.0 };
    let controls_height = 150.0 + gate_extra + karaoke_lines * 26.0 + crate::app::BOTTOM_INSET;
    let cube_size = Vec2::new(
        avail_w,
        (ui.available_height() - controls_height).max(120.0),
    );
    let now = ui.input(|i| i.time);
    if done && app.animator.is_idle() {
        if guide.done_at.is_none() {
            guide.done_at = Some(now);
            guide.bursts.push(now);
        }
    } else {
        guide.done_at = None; // stepping back re-arms the celebration
        guide.bursts.clear();
    }
    // While a move animates, the big letter and the highlighted layer
    // show THAT move; when idle they preview the next one. While GATED
    // (practice-stop), neither may leak the move.
    let gate = (!done).then(|| guide.gate()).flatten();
    let highlight = if gate.is_some() {
        None
    } else if !app.animator.is_idle() {
        guide.cursor.checked_sub(1).map(|i| guide.solution.0[i])
    } else if !done {
        Some(guide.solution.0[guide.cursor])
    } else {
        None
    };
    let cube_resp = CubeView {
        cube: &app.cube,
        animator: &app.animator,
        orbit: &mut app.orbit,
        highlight,
        dim_others: if done { 0.0 } else { 0.35 },
        color_override: None,
        hint: None,
    }
    .show(ui, cube_size);
    if guide.done_at.is_some() {
        // Tapping the solved cube fires MORE confetti.
        if cube_resp.clicked() {
            guide.bursts.push(now);
        }
        guide.bursts.retain(|&t0| now - t0 < 3.0);
        if guide.bursts.len() > 8 {
            let drop = guide.bursts.len() - 8;
            guide.bursts.drain(..drop);
        }
        for (bi, &t0) in guide.bursts.iter().enumerate() {
            confetti(ui, cube_resp.rect, now - t0, bi as u64);
        }
        if !guide.bursts.is_empty() {
            ui.ctx().request_repaint();
        }
    }

    ui.vertical_centered(|ui| {
        // Karaoke letters carry both the plan and the progress; cursor
        // counts enqueued moves, so the marker lights when a move STARTS.
        // In practice-stop, unrevealed algorithm moves render as dots.
        let hidden: Vec<bool> = guide
            .seg_id
            .iter()
            .map(|sid| {
                guide.practice
                    && sid.is_some_and(|k| !guide.revealed.contains(&k))
            })
            .collect();
        crate::widgets::playback::karaoke_row_masked(
            ui,
            &guide.solution.0,
            guide.cursor,
            !app.animator.is_idle(),
            guide.practice.then_some(hidden.as_slice()),
        );

        // «Min vei»: predicted total human time for the whole plan.
        if let Some(ms) = guide.est_ms {
            ui.colored_label(
                Color32::from_rgb(0xC7, 0x8A, 0x08),
                RichText::new(format!("★ ≈ {} s", ms.div_ceil(1000))).size(18.0),
            );
        }
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

        // Current / next move in large type — or, while GATED, the
        // practice prompt (the move stays secret).
        if done && app.animator.is_idle() {
            ui.colored_label(
                Color32::from_rgb(0x4C, 0xD9, 0x64),
                RichText::new(format!("✔ {}", app.t(TextKey::GuideDone))).size(34.0),
            );
        } else if gate.is_some() {
            ui.colored_label(
                Color32::from_rgb(0x4C, 0xD9, 0x64),
                RichText::new(app.t(TextKey::PracticeYouKnow)).size(28.0).strong(),
            );
        } else if let Some(m) = highlight {
            ui.label(RichText::new(m.to_string()).size(40.0).strong());
        } else {
            ui.label(RichText::new(" ").size(40.0));
        }
        // "Needed a peek": the cursor is inside a segment the user asked
        // to have revealed.
        if let Some(Some(k)) = guide.seg_id.get(guide.cursor).copied() {
            if guide.practice && guide.revealed.contains(&k) && !done {
                ui.colored_label(
                    Color32::from_rgb(0xE0, 0x8A, 0x1E),
                    RichText::new(app.t(TextKey::NeededHelp)).size(16.0),
                );
            }
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
            if let Some(k) = gate {
                // GATED: big green "I did it" (fast-forwards the segment
                // — the physical cube is already there) + small "Show me"
                // which reveals the letters (counts as needing help).
                let did_size = Vec2::new(next_size.x * 1.2, size.y);
                ui.add_space(
                    (ui.available_width() - (did_size.x + spacing) - (small.x + spacing))
                        .max(0.0)
                        / 2.0,
                );
                if icons::big_icon_button(
                    ui,
                    did_size,
                    Color32::from_rgb(0x1E, 0x88, 0x50),
                    app.t(TextKey::IDidIt),
                    icons::draw_check,
                )
                .clicked()
                    && app.animator.is_idle()
                {
                    // NEVER animator.clear() here: an in-flight move would
                    // be dropped unapplied while cursor counts it.
                    let end = guide.seg_end(k);
                    for &m in &guide.solution.0[guide.cursor..end] {
                        app.cube.apply(m);
                    }
                    guide.cursor = end;
                }
                if icons::big_icon_button(
                    ui,
                    small,
                    Color32::from_gray(70),
                    app.t(TextKey::ShowSolution),
                    icons::draw_play,
                )
                .clicked()
                {
                    guide.revealed.insert(k);
                }
                return;
            }
            ui.add_space(
                (ui.available_width() - 2.0 * (small.x + spacing) - (next_size.x + spacing))
                    .max(0.0)
                    / 2.0,
            );
            crate::widgets::playback::speed_buttons_sized(app, ui, small);
            if done {
                // Solved: the big button becomes the checkmark home.
                if icons::big_icon_button(
                    ui,
                    next_size,
                    Color32::from_rgb(0x1E, 0x88, 0x50),
                    "",
                    icons::draw_check,
                )
                .clicked()
                {
                    *next = Some(Screen::Menu);
                }
            } else if icons::big_icon_button(
                ui,
                next_size,
                Color32::from_rgb(0x1E, 0x88, 0x50),
                app.t(TextKey::Next),
                icons::draw_next_arrow,
            )
            .clicked()
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


/// Confetti cannon: two bursts from the bottom corners, deterministic
/// per-particle physics (no RNG at draw time — pure f(t), replayable).
fn confetti(ui: &Ui, rect: egui::Rect, t: f64, burst: u64) {
    const DURATION: f64 = 3.0;
    if !(0.0..DURATION).contains(&t) {
        return;
    }
    let p = ui.painter();
    let colors = [
        Color32::from_rgb(0xF5, 0xF5, 0xF5),
        Color32::from_rgb(0xFF, 0xD5, 0x00),
        Color32::from_rgb(0xE0, 0x1B, 0x2E),
        Color32::from_rgb(0xFF, 0x61, 0x00),
        Color32::from_rgb(0x00, 0xA8, 0x60),
        Color32::from_rgb(0x0D, 0x5C, 0xC7),
    ];
    let h = rect.height();
    let g = 1.6 * h; // px/s^2
    for i in 0..90u64 {
        // Cheap deterministic hash -> per-particle parameters (varied
        // per burst so every tap looks fresh).
        let mut z = (i + burst * 97)
            .wrapping_mul(0x9E37_79B9_7F4A_7C15)
            .wrapping_add(0xBF58_476D);
        let mut rnd = || {
            z ^= z >> 27;
            z = z.wrapping_mul(0x94D0_49BB_1331_11EB);
            (z >> 40) as f32 / 16_777_216.0
        };
        let left = i % 2 == 0;
        let origin = if left {
            egui::Pos2::new(rect.left() + 8.0, rect.bottom())
        } else {
            egui::Pos2::new(rect.right() - 8.0, rect.bottom())
        };
        // Launch 55-85 deg upward, tilted inward.
        let ang = (55.0 + 30.0 * rnd()).to_radians();
        let dir_x = if left { ang.cos() } else { -ang.cos() };
        let speed = h * (0.9 + 0.7 * rnd());
        let delay = f64::from(rnd() * 0.5);
        let tp = (t - delay).max(0.0) as f32;
        if tp <= 0.0 {
            continue;
        }
        let x = origin.x + dir_x * speed * tp;
        let y = origin.y - ang.sin() * speed * tp + 0.5 * g * tp * tp;
        let fade = (1.0 - ((t - delay) / (DURATION - delay)) as f32).clamp(0.0, 1.0);
        let color = colors[(i % 6) as usize].gamma_multiply(fade);
        // Small spinning quad.
        let sz = 3.5 + 4.0 * rnd();
        let spin = tp * (2.0 + 4.0 * rnd()) + rnd() * 6.28;
        let (sa, ca) = spin.sin_cos();
        let c = egui::Pos2::new(x, y);
        let quad = vec![
            egui::Pos2::new(c.x + ca * sz - sa * sz * 0.6, c.y + sa * sz + ca * sz * 0.6),
            egui::Pos2::new(c.x - sa * sz * 0.6 - ca * sz, c.y + ca * sz * 0.6 - sa * sz),
            egui::Pos2::new(c.x - ca * sz + sa * sz * 0.6, c.y - sa * sz - ca * sz * 0.6),
            egui::Pos2::new(c.x + sa * sz * 0.6 + ca * sz, c.y - ca * sz * 0.6 + sa * sz),
        ];
        p.add(egui::Shape::convex_polygon(quad, color, egui::Stroke::NONE));
    }
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
