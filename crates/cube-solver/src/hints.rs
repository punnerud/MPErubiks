//! Hint engine: prefer the user's trained algorithms when solving.
//!
//! Two layers:
//! 1. **Inline hints** (cheap, always on): walk the base solution's
//!    intermediate states and flag every point where a trained case
//!    applies — "you could use T-Perm here".
//! 2. **Guided solution** (bounded search): chain trained algorithms
//!    (F2L → OLL → PLL, depth ≤ 3) with kewb filling the gaps, and pick
//!    the chain that uses the most trained algorithms within a small move
//!    budget over the optimal solution. Pedagogically better, not shorter.

use crate::solve::{solve, solve_bounded};
use crate::SolveError;
use cube_core::{Alg, CaseSet, Face, FaceletCube, Match, Move, Recognizer, Turns};

#[derive(Clone, Debug)]
pub struct HintAt {
    /// Index into the base solution: the hint applies BEFORE move `step`
    /// (0 = at the scanned state).
    pub step: usize,
    pub matched: Match,
}

#[derive(Clone, Debug)]
pub enum Segment {
    /// Filler moves from the solver.
    Raw(Alg),
    /// A trained algorithm, ready to execute (pre-AUF and frame folded in).
    Trained { case_idx: u16, exec: Alg },
}

impl Segment {
    pub fn alg(&self) -> &Alg {
        match self {
            Segment::Raw(a) => a,
            Segment::Trained { exec, .. } => exec,
        }
    }
}

#[derive(Clone, Debug)]
pub struct GuidedSolution {
    pub segments: Vec<Segment>,
    pub total_htm: usize,
    pub trained_used: usize,
    /// Ergonomic cost of the whole solution: moves are "paths" and not all
    /// paths are equal — R/U turns are cheapest in the hand, D/B awkward,
    /// off-frame matches cost a regrip, and untrained filler moves weigh
    /// more than practiced algorithm moves. Lower = smoother to perform.
    pub ergo_cost: u32,
    /// Ergonomic cost spent BEFORE the first trained segment: 0 means the
    /// solution opens with something the user knows (placement objective —
    /// demonstrate knowledge as early as possible).
    pub first_trained_ergo: u32,
}

/// Ergonomic cost of physically performing a sequence, ~10 per comfortable
/// quarter turn. Face weights follow speedcubing ergonomics (R/U flow, B is
/// a regrip); half turns cost 1.5x; whole-cube rotations move no pieces but
/// cost a regrip.
fn ergonomic_cost(alg: &Alg) -> u32 {
    alg.0
        .iter()
        .map(|m| {
            let (base, turns) = match m {
                Move::Face(Face::U | Face::R, t) => (10, *t),
                Move::Face(Face::F | Face::L, t) => (12, *t),
                Move::Face(Face::D, t) => (14, *t),
                Move::Face(Face::B, t) => (16, *t),
                Move::Wide(_, t) => (14, *t),
                Move::Slice(_, t) => (15, *t),
                Move::Rot(_, t) => (8, *t),
            };
            if matches!(turns, Turns::Half) {
                base * 3 / 2
            } else {
                base
            }
        })
        .sum()
}

/// Regrip cost of executing a match in a rotated y frame: the user works
/// "from a different angle" — one quarter of reorientation per step away
/// from their current front.
fn frame_regrip_cost(y_frame: u8) -> u32 {
    let j = u32::from(y_frame % 4);
    j.min(4 - j) * 8
}

/// Filler (kewb) moves are unpracticed reading-and-turning; trained
/// algorithm moves are muscle memory. Weight filler 25% heavier.
fn filler_ergo(alg: &Alg) -> u32 {
    ergonomic_cost(alg) * 5 / 4
}

/// Every algorithm start costs a "look": the human pauses to recognize
/// the case before the hands move. Roughly one comfortable turn.
const RECOGNITION_PAUSE: i64 = 8;

fn single_move_ergo(m: Move) -> i64 {
    i64::from(ergonomic_cost(&Alg::new(vec![m])))
}

/// Junction cost between two consecutive segments — the routing "turn
/// penalty", computed from the algs themselves (no hand-kept ID matrix):
///
/// - **Cancellation bonus** (negative): same-face moves at the seam merge
///   (`R' | R` vanishes entirely, `R | R` becomes one `R2`) — the classic
///   FMC trick, rewarded so chains that flow into each other win.
/// - **Grip penalty** (positive): awkward hand-position changes at the
///   seam (B is a regrip for most grips, D a wrist turn, F↔L crossings).
fn junction_cost(prev_last: Option<Move>, next_first: Option<Move>) -> i64 {
    let (Some(prev), Some(next)) = (prev_last, next_first) else {
        return 0;
    };
    let (Move::Face(pf, pt), Move::Face(nf, nt)) = (prev, next) else {
        return 0; // seams with slice/wide/rot moves: no model yet
    };

    if pf == nf {
        // Merge at the seam: quarters add mod 4.
        let q = (pt.quarters() + nt.quarters()) % 4;
        let separate = single_move_ergo(prev) + single_move_ergo(next);
        let merged = match q {
            0 => 0, // full cancellation: both moves vanish
            1 => single_move_ergo(Move::Face(pf, Turns::Cw)),
            2 => single_move_ergo(Move::Face(pf, Turns::Half)),
            _ => single_move_ergo(Move::Face(pf, Turns::Ccw)),
        };
        return merged - separate; // <= 0: never worse than executing both
    }

    // Grip discontinuities at the seam.
    let involves = |f: Face| pf == f || nf == f;
    let mut penalty = 0;
    if involves(Face::B) {
        penalty += 6;
    }
    if involves(Face::D) {
        penalty += 3;
    }
    if (pf == Face::F && nf == Face::L) || (pf == Face::L && nf == Face::F) {
        penalty += 2;
    }
    penalty
}

#[derive(Clone, Debug)]
pub struct SolveOutput {
    /// The plain optimal-ish solution (≤ 21, fallback 23).
    pub base: Alg,
    pub inline_hints: Vec<HintAt>,
    /// Present when at least one trained algorithm can be woven in within
    /// the move budget.
    pub guided: Option<GuidedSolution>,
}

const ALL_SETS: [CaseSet; 4] = [CaseSet::F2l, CaseSet::Oll, CaseSet::Pll, CaseSet::Lbl];
const MAX_CHAIN_DEPTH: usize = 3;
const MAX_FRONTIER: usize = 12;

pub fn solve_with_hints(
    s: &FaceletCube,
    trained: &[u16],
    rec: &Recognizer,
) -> Result<SolveOutput, SolveError> {
    let base = solve(s)?;
    let inline_hints = collect_inline_hints(s, &base, trained, rec);
    let guided = guided_solution(s, &base, trained, rec)?;
    Ok(SolveOutput {
        base,
        inline_hints,
        guided,
    })
}

fn trained_matches(s: &FaceletCube, trained: &[u16], rec: &Recognizer) -> Vec<Match> {
    rec.recognize(s, &ALL_SETS)
        .into_iter()
        .filter(|m| trained.contains(&m.case_idx))
        .collect()
}

fn collect_inline_hints(
    s: &FaceletCube,
    base: &Alg,
    trained: &[u16],
    rec: &Recognizer,
) -> Vec<HintAt> {
    let mut out = Vec::new();
    let mut state = *s;
    for step in 0..=base.0.len() {
        for matched in trained_matches(&state, trained, rec) {
            out.push(HintAt { step, matched });
        }
        if step < base.0.len() {
            state.apply(base.0[step]);
        }
    }
    out
}

struct Node {
    state: FaceletCube,
    segments: Vec<Segment>,
    used_htm: usize,
    /// Accumulated ergonomic cost (trained segments + setups + regrips).
    ergo: u32,
    /// Ergo spent before the first trained segment (None until one is used).
    first_trained_ergo: Option<u32>,
    trained_used: usize,
}

fn guided_solution(
    s: &FaceletCube,
    base: &Alg,
    trained: &[u16],
    rec: &Recognizer,
) -> Result<Option<GuidedSolution>, SolveError> {
    if trained.is_empty() {
        return Ok(None);
    }
    let budget = (base.len_htm() + 6).max(24);

    // Tail-solve memo: different chains often converge to the same state
    // (e.g. two different OLL paths reaching the same PLL). A kewb search
    // is the expensive "cell"; buy it once. Value = (bound tried, result).
    let mut tail_memo: std::collections::HashMap<FaceletCube, (usize, Option<Alg>)> =
        std::collections::HashMap::new();

    let mut frontier = vec![Node {
        state: *s,
        segments: Vec::new(),
        used_htm: 0,
        ergo: 0,
        first_trained_ergo: None,
        trained_used: 0,
    }];
    let mut best: Option<GuidedSolution> = None;

    for _depth in 0..MAX_CHAIN_DEPTH {
        let mut next = Vec::new();
        for node in &frontier {
            let mut children = Vec::new();
            // Direct matches, plus matches enabled by ONE setup move —
            // "R makes your T-Perm applicable" teaches more than a raw
            // filler sequence would. (U-turn setups are already folded
            // into matches as pre-AUF, so setups skip the U face.)
            let prev_last_move = node.segments.last().and_then(|s| s.alg().0.last().copied());
            let mut consider = |setup: Option<Move>, state: &FaceletCube| {
                for m in trained_matches(state, trained, rec) {
                    // Reduce to face moves: rotation-free by construction,
                    // so segments chain in one fixed frame and the kewb
                    // tail always sees home-oriented centers.
                    let exec = rec.execution_alg(m).face_moves_only();
                    let cost = exec.len_htm() + usize::from(setup.is_some());
                    if node.used_htm + cost > budget {
                        continue;
                    }
                    // Path cost: the setup is unpracticed (filler weight),
                    // the alg itself is muscle memory, matching in a
                    // rotated frame costs a regrip, and every seam pays a
                    // junction cost (cancellation bonus / grip penalty).
                    let setup_ergo = setup
                        .map(|mv| i64::from(filler_ergo(&Alg::new(vec![mv]))))
                        .unwrap_or(0);
                    let seam_before_exec = match setup {
                        Some(mv) => {
                            junction_cost(prev_last_move, Some(mv))
                                + junction_cost(Some(mv), exec.0.first().copied())
                        }
                        None => junction_cost(prev_last_move, exec.0.first().copied()),
                    };
                    let pre_exec_ergo =
                        (i64::from(node.ergo) + setup_ergo + seam_before_exec).max(0);
                    let exec_ergo = i64::from(ergonomic_cost(&exec))
                        + i64::from(frame_regrip_cost(m.y_frame))
                        + RECOGNITION_PAUSE;
                    let child_state = state.applied_alg(&exec).normalize_orientation();
                    let mut segments = node.segments.clone();
                    if let Some(setup) = setup {
                        segments.push(Segment::Raw(Alg::new(vec![setup])));
                    }
                    segments.push(Segment::Trained {
                        case_idx: m.case_idx,
                        exec,
                    });
                    children.push(Node {
                        state: child_state,
                        segments,
                        used_htm: node.used_htm + cost,
                        ergo: (pre_exec_ergo + exec_ergo).max(0) as u32,
                        first_trained_ergo: node
                            .first_trained_ergo
                            .or(Some((i64::from(node.ergo) + setup_ergo).max(0) as u32)),
                        trained_used: node.trained_used + 1,
                    });
                }
            };
            consider(None, &node.state);
            for mv_idx in 0..18 {
                let mv = Move::from_index(mv_idx);
                if matches!(mv, Move::Face(Face::U, _)) {
                    continue;
                }
                consider(Some(mv), &node.state.applied(mv));
            }
            // Cap per-node fan-out so one node can't flood the frontier;
            // rank by path cost, not raw move count.
            children.sort_by_key(|n| n.ergo);
            children.truncate(6);
            next.extend(children);
        }
        if next.is_empty() {
            break;
        }
        next.sort_by_key(|n| n.ergo);
        next.truncate(MAX_FRONTIER);

        // Evaluate every node with >= 1 trained segment: kewb solves the rest.
        for node in &next {
            let remaining = budget.saturating_sub(node.used_htm);
            let tail = if node.state.is_solved() {
                Some(Alg::default())
            } else if remaining == 0 {
                None
            } else {
                let bound = remaining.min(23);
                match tail_memo.get(&node.state) {
                    Some((_, Some(memo))) if memo.len_htm() <= remaining => {
                        Some(memo.clone())
                    }
                    Some((tried, None)) if bound <= *tried => None,
                    _ => {
                        let solved = match solve_bounded(&node.state, bound as u8) {
                            Ok(alg) => Some(alg),
                            Err(SolveError::NoSolution) => None,
                            Err(e) => return Err(e),
                        };
                        tail_memo.insert(node.state, (bound, solved.clone()));
                        solved
                    }
                }
            };
            let Some(tail) = tail else { continue };
            let total = node.used_htm + tail.len_htm();
            let tail_seam = junction_cost(
                node.segments.last().and_then(|s| s.alg().0.last().copied()),
                tail.0.first().copied(),
            );
            let ergo_cost =
                (i64::from(node.ergo) + i64::from(filler_ergo(&tail)) + tail_seam).max(0) as u32;
            let mut segments = node.segments.clone();
            if !tail.is_empty() {
                segments.push(Segment::Raw(tail));
            }
            let candidate = GuidedSolution {
                total_htm: total,
                trained_used: node.trained_used,
                ergo_cost,
                first_trained_ergo: node.first_trained_ergo.unwrap_or(node.ergo),
                segments,
            };
            // Objective, lexicographic (the MPEE-style multi-criteria):
            // most trained algorithms used; among those, knowledge shown
            // earliest; among those, smoothest path in the hands.
            let key = |g: &GuidedSolution| {
                (
                    std::cmp::Reverse(g.trained_used),
                    g.first_trained_ergo,
                    g.ergo_cost,
                )
            };
            if best.as_ref().is_none_or(|b| key(&candidate) < key(b)) {
                best = Some(candidate);
            }
        }
        frontier = next;
    }
    Ok(best)
}

#[cfg(test)]
mod tests {
    use super::*;
    use cube_core::{CaseDef, RecogKind, SplitMix64};

    fn ensure_table() {
        if !crate::table_ready() {
            let bytes = std::fs::read(concat!(
                env!("CARGO_MANIFEST_DIR"),
                "/../../assets/table.bin"
            ))
            .expect("assets/table.bin missing — run: cargo run -p xtask -- gen-table");
            crate::install_table(&bytes).unwrap();
        }
    }

    fn rec_with(cases: &[(&str, CaseSet, &str, RecogKind)]) -> Recognizer {
        Recognizer::new(
            cases
                .iter()
                .map(|(id, set, moves, recognition)| CaseDef {
                    id: id.to_string(),
                    set: *set,
                    name: id.to_string(),
                    group: String::new(),
                    alg: Alg::parse(moves).unwrap(),
                    recognition: *recognition,
                })
                .collect(),
        )
        .unwrap()
    }

    #[test]
    fn t_perm_state_yields_guided_solution_using_t_perm() {
        ensure_table();
        let rec = rec_with(&[(
            "pll-t",
            CaseSet::Pll,
            "R U R' U' R' F R2 U' R' U' R U R' F'",
            RecogKind::Pll,
        )]);
        let t_idx = rec.find_by_id("pll-t").unwrap();
        let mut rng = SplitMix64::new(7);
        let state = rec.setup_state(t_idx, &mut rng);

        let out = solve_with_hints(&state, &[t_idx], &rec).unwrap();
        // The hint pass must see T-Perm at step 0.
        assert!(
            out.inline_hints.iter().any(|h| h.step == 0 && h.matched.case_idx == t_idx),
            "expected an inline T-Perm hint at the start"
        );
        // The guided solution must use it and actually solve the cube.
        let guided = out.guided.expect("guided solution");
        assert_eq!(guided.trained_used, 1);
        let mut s = state;
        for seg in &guided.segments {
            s.apply_alg(seg.alg());
        }
        assert!(
            (0..4).any(|k| s.rotate_u(k).is_solved()),
            "guided solution must solve (up to final AUF)"
        );
    }

    #[test]
    fn ergonomic_cost_orders_paths_like_a_hand_would() {
        let cost = |s: &str| ergonomic_cost(&Alg::parse(s).unwrap());
        // Same move count, different hands: R/U flow beats B/D grinding.
        assert!(cost("R U R' U'") < cost("B D B' D'"));
        // Half turns cost more than quarters, less than two moves.
        assert!(cost("R2") > cost("R") && cost("R2") < cost("R R"));
        // A rotation costs something (regrip) even though no pieces move.
        assert!(cost("y") > 0);
        // Filler weighting: identical moves cost more as unpracticed filler.
        let alg = Alg::parse("R U R' U'").unwrap();
        assert!(filler_ergo(&alg) > ergonomic_cost(&alg));
        // Off-frame matches cost a regrip, symmetric around the cube.
        assert_eq!(frame_regrip_cost(0), 0);
        assert_eq!(frame_regrip_cost(1), frame_regrip_cost(3));
        assert!(frame_regrip_cost(2) > frame_regrip_cost(1));
    }

    #[test]
    fn direct_use_is_preferred_and_placement_is_reported() {
        ensure_table();
        let rec = rec_with(&[(
            "pll-t",
            CaseSet::Pll,
            "R U R' U' R' F R2 U' R' U' R U R' F'",
            RecogKind::Pll,
        )]);
        let t_idx = rec.find_by_id("pll-t").unwrap();
        let mut rng = SplitMix64::new(11);
        let state = rec.setup_state(t_idx, &mut rng);

        let guided = solve_with_hints(&state, &[t_idx], &rec)
            .unwrap()
            .guided
            .expect("guided");
        // The T-Perm applies directly: the solution must OPEN with it (no
        // setup/filler first), and say so via the placement metric.
        assert!(
            matches!(guided.segments[0], Segment::Trained { .. }),
            "solution must open with the trained algorithm"
        );
        assert_eq!(
            guided.first_trained_ergo, 0,
            "knowledge demonstrated at the very start"
        );
        assert!(guided.ergo_cost > 0);
    }

    #[test]
    fn junction_costs_model_seams_hints() {
        use cube_core::{Face, Move, Turns};
        let m = |f, t| Move::Face(f, t);
        // Full cancellation (R' | R): strongly negative — both moves vanish.
        let cancel = junction_cost(Some(m(Face::R, Turns::Ccw)), Some(m(Face::R, Turns::Cw)));
        assert!(cancel < -15, "full cancel should refund both moves, got {cancel}");
        // Merge (R | R -> R2): refunds part of the pair.
        let merge = junction_cost(Some(m(Face::R, Turns::Cw)), Some(m(Face::R, Turns::Cw)));
        assert!(merge < 0 && merge > cancel, "merge refunds less than full cancel");
        // B at the seam is a regrip; R->U flows free.
        assert!(junction_cost(Some(m(Face::F, Turns::Cw)), Some(m(Face::B, Turns::Cw))) > 0);
        assert_eq!(
            junction_cost(Some(m(Face::R, Turns::Cw)), Some(m(Face::U, Turns::Cw))),
            0
        );
        // Open seams cost nothing.
        assert_eq!(junction_cost(None, Some(m(Face::R, Turns::Cw))), 0);
    }

    #[test]
    fn setup_move_enables_trained_alg() {
        ensure_table();
        let rec = rec_with(&[(
            "pll-t",
            CaseSet::Pll,
            "R U R' U' R' F R2 U' R' U' R U R' F'",
            RecogKind::Pll,
        )]);
        let t_idx = rec.find_by_id("pll-t").unwrap();
        // State = R' applied after creating a T-perm case: solving needs
        // the setup move R first, then the trained T-Perm.
        let mut state = FaceletCube::SOLVED;
        state.apply_alg(&Alg::parse("R U R' U' R' F R2 U' R' U' R U R' F'").unwrap().inverse());
        state.apply_alg(&Alg::parse("R'").unwrap());

        let out = solve_with_hints(&state, &[t_idx], &rec).unwrap();
        let guided = out.guided.expect("guided solution with setup");
        assert_eq!(guided.trained_used, 1, "T-Perm woven in after a setup move");
        let mut s = state;
        for seg in &guided.segments {
            s.apply_alg(seg.alg());
        }
        assert!((0..4).any(|k| s.rotate_u(k).is_solved()));
    }

    #[test]
    fn chain_oll_then_pll() {
        ensure_table();
        let rec = rec_with(&[
            ("oll-27", CaseSet::Oll, "R U R' U R U2 R'", RecogKind::Oll),
            (
                "pll-t",
                CaseSet::Pll,
                "R U R' U' R' F R2 U' R' U' R U R' F'",
                RecogKind::Pll,
            ),
        ]);
        let sune = rec.find_by_id("oll-27").unwrap();
        let t = rec.find_by_id("pll-t").unwrap();
        // Build a state that is exactly inverse(Sune) after inverse(T-perm):
        // solving needs Sune, then T-perm.
        let mut state = FaceletCube::SOLVED;
        state.apply_alg(&Alg::parse("R U R' U' R' F R2 U' R' U' R U R' F'").unwrap().inverse());
        state.apply_alg(&Alg::parse("R U R' U R U2 R'").unwrap().inverse());

        let out = solve_with_hints(&state, &[sune, t], &rec).unwrap();
        let guided = out.guided.expect("guided solution");
        assert_eq!(guided.trained_used, 2, "should chain Sune then T-Perm");
        let mut s = state;
        for seg in &guided.segments {
            s.apply_alg(seg.alg());
        }
        assert!((0..4).any(|k| s.rotate_u(k).is_solved()));
    }
}
