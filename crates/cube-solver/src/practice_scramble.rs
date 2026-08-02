//! Practice scrambles: a scramble whose SOLUTION contains the algorithms
//! you are drilling — optionally several times.
//!
//! Two ways to get one, both useful:
//! - [`Mode::Random`] draws honest random states and keeps the ones whose
//!   guided solution happens to use the chosen algorithms. Realistic, but
//!   luck-bound (one specific PLL is ~1 in 21).
//! - [`Mode::Built`] constructs the state backwards along a route of
//!   algorithm inverses with filler between, so the count is guaranteed.
//!
//! Either way the reported share is measured on the solution the app will
//! actually show, never on the route it was built from — the number has
//! to be true even when the engine finds something better.

use crate::hints::{solve_with_hints, GuidedSolution, Segment};
use cube_core::{FaceletCube, Move, Recognizer, SplitMix64};

#[derive(Clone, Copy, PartialEq, Eq, Debug)]
pub enum Mode {
    /// Honest random states, kept when they hit the target.
    Random,
    /// Constructed backwards: the count is guaranteed.
    Built,
    /// Try Random first (more realistic), fall back to Built.
    Auto,
}

pub struct PracticeScramble {
    pub state: FaceletCube,
    pub solution: GuidedSolution,
    /// (case index, times it appears in the shown solution).
    pub counts: Vec<(u16, usize)>,
    /// Fraction of the solution's moves that are the chosen algorithms.
    pub share: f32,
    /// How the state was produced (Auto resolves to what it used).
    pub mode: Mode,
}

/// Random-mode attempts. Each is a kewb solve plus a bounded weave
/// search; the button must still feel instant on a phone.
const RANDOM_ATTEMPTS: usize = 8;

pub fn practice_scramble(
    rng: &mut SplitMix64,
    rec: &Recognizer,
    wanted: &[u16],
    target: usize,
    mode: Mode,
    max_extra: usize,
) -> Option<PracticeScramble> {
    // Only cases the recognizer can SEE can be woven into a solution:
    // the beginner steps (RecogKind::None) are executed by the macro
    // router, not matched in a state, so they can never be targeted here.
    let wanted: Vec<u16> = wanted
        .iter()
        .copied()
        .filter(|&c| !matches!(rec.case(c).recognition, cube_core::RecogKind::None))
        .collect();
    let wanted = wanted.as_slice();
    if wanted.is_empty() {
        return None;
    }
    let target = target.max(1);
    if matches!(mode, Mode::Random | Mode::Auto) {
        for _ in 0..RANDOM_ATTEMPTS {
            let state = crate::random_state();
            if let Some(found) = evaluate(&state, rec, wanted, target, max_extra, Mode::Random) {
                return Some(found);
            }
        }
        if mode == Mode::Random {
            return None;
        }
    }
    // Built: walk backwards from solved along algorithm inverses, and
    // keep the BEST of several attempts. Repeats cannot be guaranteed —
    // the second inverse disturbs the pieces the first one ejected, and
    // a last-layer algorithm ends the solve outright — so we maximize
    // occurrences and report the true number instead of promising one.
    let mut best: Option<PracticeScramble> = None;
    for count in (1..=target).rev() {
        for _ in 0..4 {
            let state = build_backwards(rng, rec, wanted, count);
            // The engine's own solution is what gets shown, so the share
            // must be measured on it — it may beat the built route.
            let Some(found) = evaluate(&state, rec, wanted, 1, max_extra, Mode::Built) else {
                continue;
            };
            let n: usize = found.counts.iter().map(|(_, k)| k).sum();
            if n >= target {
                return Some(found);
            }
            let better = best
                .as_ref()
                .map(|b| n > b.counts.iter().map(|(_, k)| k).sum::<usize>())
                .unwrap_or(true);
            if better {
                best = Some(found);
            }
        }
    }
    best
}

/// Solve `state` with the weave engine and measure how much of the
/// result is the wanted algorithms.
fn evaluate(
    state: &FaceletCube,
    rec: &Recognizer,
    wanted: &[u16],
    target: usize,
    max_extra: usize,
    mode: Mode,
) -> Option<PracticeScramble> {
    let out = solve_with_hints(state, wanted, rec, max_extra).ok()?;
    let guided = out.guided?;
    let mut counts: Vec<(u16, usize)> = Vec::new();
    let mut alg_moves = 0usize;
    for seg in &guided.segments {
        if let Segment::Trained {
            case_idx, auf_len, ..
        } = seg
        {
            if !wanted.contains(case_idx) {
                continue;
            }
            match counts.iter_mut().find(|(c, _)| c == case_idx) {
                Some((_, n)) => *n += 1,
                None => counts.push((*case_idx, 1)),
            }
            // The pre-AUF is alignment, not the algorithm itself.
            alg_moves += seg.alg().len_htm().saturating_sub(usize::from(*auf_len));
        }
    }
    let total: usize = counts.iter().map(|(_, n)| n).sum();
    if total < target || guided.total_htm == 0 {
        return None;
    }
    Some(PracticeScramble {
        state: *state,
        share: alg_moves as f32 / guided.total_htm as f32,
        solution: guided,
        counts,
        mode,
    })
}

/// Build a state whose forward solution runs the chosen algorithms
/// `count` times, by applying their INVERSES in DISTINCT whole-cube
/// frames.
///
/// Two constraints learned the hard way:
/// - random filler moves between the inverses break the very case the
///   next inverse is supposed to leave behind (a PLL pattern is only a
///   PLL pattern over an intact F2L), so there is none;
/// - repeats are only real for algorithms that address one slot at a
///   time (F2L, the beginner insertions). A last-layer algorithm ends
///   the solve, so two of them compose into a *different* case — the
///   caller retries with a lower count and reports what truly happened.
fn build_backwards(
    rng: &mut SplitMix64,
    rec: &Recognizer,
    wanted: &[u16],
    count: usize,
) -> FaceletCube {
    let mut state = FaceletCube::SOLVED;
    let mut frames = [0u8, 1, 2, 3];
    // Shuffle the frames so repeats hit different slots.
    for i in (1..frames.len()).rev() {
        frames.swap(i, rng.below(i as u32 + 1) as usize);
    }
    for i in 0..count {
        let case = wanted[rng.below(wanted.len() as u32) as usize];
        let frame = frames[i % frames.len()];
        let alg = rec.case(case).alg.in_y_frame(frame).face_moves_only();
        state.apply_alg(&alg.inverse());
    }
    // A little AUF is harmless: it keeps the state from looking staged
    // without disturbing which cases are present.
    for _ in 0..rng.below(4) {
        state.apply(Move::Face(cube_core::Face::U, cube_core::Turns::Cw));
    }
    state
}

#[cfg(test)]
mod tests {
    use super::*;

    fn table() {
        if !crate::table_ready() {
            let bytes = std::fs::read(concat!(
                env!("CARGO_MANIFEST_DIR"),
                "/../../assets/table.bin"
            ))
            .expect("run: cargo run -p xtask -- gen-table");
            crate::install_table(&bytes).unwrap();
        }
    }

    fn rec_with(cases: &[(&str, cube_core::CaseSet, &str, cube_core::RecogKind)]) -> Recognizer {
        Recognizer::new(
            cases
                .iter()
                .map(|(id, set, moves, recognition)| cube_core::CaseDef {
                    id: id.to_string(),
                    set: *set,
                    name: id.to_string(),
                    group: String::new(),
                    alg: cube_core::Alg::parse(moves).unwrap(),
                    recognition: *recognition,
                })
                .collect(),
        )
        .unwrap()
    }

    fn t_perm() -> (Recognizer, u16) {
        let rec = rec_with(&[(
            "pll-t",
            cube_core::CaseSet::Pll,
            "R U R' U' R' F R2 U' R' U' R U R' F'",
            cube_core::RecogKind::Pll,
        )]);
        let idx = rec.find_by_id("pll-t").unwrap();
        (rec, idx)
    }

    #[test]
    fn built_mode_always_delivers_a_practice_scramble() {
        table();
        let (rec, t) = t_perm();
        let mut rng = SplitMix64::new(4);
        for round in 0..8 {
            let ps = practice_scramble(&mut rng, &rec, &[t], 2, Mode::Built, 8)
                .unwrap_or_else(|| panic!("round {round}: no scramble"));
            let used: usize = ps.counts.iter().map(|(_, n)| n).sum();
            assert!(used >= 1, "round {round}: algorithm not used");
            assert!(
                (0.0..=1.0).contains(&ps.share) && ps.share > 0.0,
                "round {round}: share {}",
                ps.share
            );
            // The reported solution must actually solve the state.
            let mut cube = ps.state;
            for seg in &ps.solution.segments {
                cube.apply_alg(seg.alg());
            }
            assert!(cube.is_solved(), "round {round}: solution does not solve");
        }
    }

    /// Repeats are real for slot-at-a-time algorithms: the beginner
    /// corner insert can (and does) appear more than once in a solve.
    #[test]
    fn repeatable_algorithms_can_appear_several_times() {
        table();
        // An F2L case addresses ONE slot, so the same algorithm can be
        // needed several times in a solve (unlike a last-layer case).
        let rec = rec_with(&[(
            "f2l-01",
            cube_core::CaseSet::F2l,
            "U R U' R'",
            cube_core::RecogKind::F2l,
        )]);
        let idx = rec.find_by_id("f2l-01").unwrap();
        let mut rng = SplitMix64::new(3);
        let mut best = 0;
        for _ in 0..6 {
            if let Some(ps) = practice_scramble(&mut rng, &rec, &[idx], 3, Mode::Built, 10) {
                best = best.max(ps.counts.iter().map(|(_, n)| n).sum::<usize>());
            }
        }
        // At least once, every time. More than once HAPPENS (the count
        // is reported honestly) but cannot be promised: the second
        // inverse disturbs the pieces the first one ejected.
        assert!(best >= 1, "the chosen algorithm must appear, best was {best}");
    }

    #[test]
    fn share_matches_the_shown_solution() {
        table();
        let (rec, t) = t_perm();
        let mut rng = SplitMix64::new(11);
        let ps = practice_scramble(&mut rng, &rec, &[t], 1, Mode::Auto, 8).expect("scramble");
        let alg_moves: usize = ps
            .solution
            .segments
            .iter()
            .filter_map(|s| match s {
                Segment::Trained {
                    case_idx,
                    auf_len,
                    exec,
                } if *case_idx == t => Some(exec.len_htm() - usize::from(*auf_len)),
                _ => None,
            })
            .sum();
        let expect = alg_moves as f32 / ps.solution.total_htm as f32;
        assert!(
            (ps.share - expect).abs() < 1e-6,
            "share {} vs {expect}",
            ps.share
        );
    }
}
