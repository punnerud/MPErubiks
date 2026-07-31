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
use cube_core::{Alg, CaseSet, FaceletCube, Match, Recognizer};

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

    let mut frontier = vec![Node {
        state: *s,
        segments: Vec::new(),
        used_htm: 0,
        trained_used: 0,
    }];
    let mut best: Option<GuidedSolution> = None;

    for _depth in 0..MAX_CHAIN_DEPTH {
        let mut next = Vec::new();
        for node in &frontier {
            for m in trained_matches(&node.state, trained, rec) {
                let exec = rec.execution_alg(m);
                let cost = exec.len_htm();
                if node.used_htm + cost > budget {
                    continue;
                }
                let child_state = node.state.applied_alg(&exec).normalize_orientation();
                let mut segments = node.segments.clone();
                segments.push(Segment::Trained {
                    case_idx: m.case_idx,
                    exec,
                });
                next.push(Node {
                    state: child_state,
                    segments,
                    used_htm: node.used_htm + cost,
                    trained_used: node.trained_used + 1,
                });
            }
        }
        if next.is_empty() {
            break;
        }
        next.sort_by_key(|n| n.used_htm);
        next.truncate(MAX_FRONTIER);

        // Evaluate every node with >= 1 trained segment: kewb solves the rest.
        for node in &next {
            let remaining = budget.saturating_sub(node.used_htm);
            let tail = if node.state.is_solved() {
                Some(Alg::default())
            } else if remaining == 0 {
                None
            } else {
                match solve_bounded(&node.state, remaining.min(23) as u8) {
                    Ok(alg) => Some(alg),
                    Err(SolveError::NoSolution) => None,
                    Err(e) => return Err(e),
                }
            };
            let Some(tail) = tail else { continue };
            let total = node.used_htm + tail.len_htm();
            let mut segments = node.segments.clone();
            if !tail.is_empty() {
                segments.push(Segment::Raw(tail));
            }
            let candidate = GuidedSolution {
                total_htm: total,
                trained_used: node.trained_used,
                segments,
            };
            let better = match &best {
                None => true,
                Some(b) => {
                    (candidate.trained_used, std::cmp::Reverse(candidate.total_htm))
                        > (b.trained_used, std::cmp::Reverse(b.total_htm))
                }
            };
            if better {
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
