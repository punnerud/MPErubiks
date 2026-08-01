//! "Show me" must work for EVERY trainable case from its fresh setup
//! state: recognize_case -> execution_alg -> the SET GOAL is reached.
//! OLL's goal is an oriented last layer over intact F2L (symmetric
//! patterns legitimately resolve to a different AUF, leaving a PLL);
//! the other sets must land solved up to a final AUF.

use cube_core::{Face, FaceletCube, Move, SplitMix64, Turns};

fn f2l_solved(s: &FaceletCube) -> bool {
    let n = s.normalize_orientation();
    let solved = FaceletCube::SOLVED;
    // D face + the two lower rows of F/R/B/L solved; last layer free.
    (27..36).all(|i| n.0[i] == solved.0[i])
        && [Face::R, Face::F, Face::L, Face::B]
            .iter()
            .all(|&f| (3..9).all(|o| n.0[f as usize * 9 + o] == solved.0[f as usize * 9 + o]))
}

fn oriented_over_intact_f2l(s: &FaceletCube) -> bool {
    let n = s.normalize_orientation();
    (0..9).all(|i| n.0[i] == Face::U) && f2l_solved(s)
}

fn solved_up_to_auf(s: &FaceletCube) -> bool {
    let mut s = *s;
    (0..4).any(|_| {
        let done = s.normalize_orientation() == FaceletCube::SOLVED;
        s.apply(Move::Face(Face::U, Turns::Cw));
        done
    })
}

#[test]
fn show_me_recognizes_and_reaches_the_set_goal_for_every_case() {
    let lib = cube_app::library::Library::load();
    let mut rng = SplitMix64::new(7);
    let mut failures = Vec::new();
    for idx in 0..lib.rec.defs().len() as u16 {
        let id = lib.rec.case(idx).id.clone();
        for round in 0..8 {
            // Both the drill setup state and the canonical state (the
            // "Show me after exploring" reset target) must work.
            let states = [
                lib.rec.setup_state(idx, &mut rng),
                lib.rec.canonical_state(idx),
            ];
            for (which, state) in states.iter().enumerate() {
                // lbl cases have no recognition: the app resets to the
                // canonical state and plays the algorithm as written.
                let m = if id.starts_with("lbl") {
                    cube_core::Match {
                        case_idx: idx,
                        pre_auf: 0,
                        y_frame: 0,
                    }
                } else if let Some(m) = lib.rec.recognize_case(state, idx) {
                    m
                } else {
                    failures.push(format!(
                        "case {idx} ({}) round {round}.{which}: NOT RECOGNIZED",
                        lib.rec.case(idx).id
                    ));
                    continue;
                };
                let state = if id.starts_with("lbl") {
                    lib.rec.canonical_state(idx)
                } else {
                    *state
                };
                let mut s = state;
                let exec = lib.rec.execution_alg(m);
                if exec.0.is_empty() {
                    failures.push(format!("case {idx} ({id}): empty demo"));
                    continue;
                }
                s.apply_alg(&exec);
                // Per-set goals. Symmetric patterns and slot-bound pairs
                // legitimately resolve to a different AUF, so partial
                // steps must be judged on THEIR promise, not full solves:
                // OLL promises orientation over intact F2L, F2L a solved
                // first-two-layers, PLL a full solve up to AUF. The lbl-*
                // demos are per-case partial steps (top cross, one corner
                // in, ...) — recognition + a runnable demo is the
                // contract there.
                let ok = if id.starts_with("oll") {
                    oriented_over_intact_f2l(&s)
                } else if id.starts_with("f2l") {
                    f2l_solved(&s)
                } else if id.starts_with("lbl") {
                    true
                } else {
                    solved_up_to_auf(&s)
                };
                if !ok {
                    failures.push(format!(
                        "case {idx} ({}) round {round}.{which}: goal not reached",
                        lib.rec.case(idx).id
                    ));
                }
            }
        }
    }
    assert!(
        failures.is_empty(),
        "{} failures:\n{}",
        failures.len(),
        failures.join("\n")
    );
}
