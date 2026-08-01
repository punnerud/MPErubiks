//! Constraint-resolution of a scanned cube: "analyze your way to correct".
//!
//! The camera delivers vote-share EVIDENCE per facelet, not certainties.
//! The cube itself is a hard constraint system — exactly nine stickers of
//! each color, every one of the 26 physical pieces exactly once, twist/
//! flip/parity solvable — so uncertain cells don't need better optics:
//! rank each cell's candidates by votes and let the constraints choose.
//! If even the confident assignment is invalid, progressively free the
//! lowest-margin cells and search their alternatives.

use crate::validate;
use cube_core::{Face, FaceletCube};

/// Vote share per class, already mapped into FACE space (class -> Face by
/// the capture order), indexed by facelet.
pub type Shares = [[f32; 6]; 54];

/// A cell is treated as UNCERTAIN below this winning share.
const CONFIDENT_SHARE: f32 = 0.10;
/// Cells with NO usable evidence (top share below EVIDENCE_FLOOR) are
/// fully unconstrained and try all six colors.
const EVIDENCE_FLOOR: f32 = 0.05;
/// A class is a PLAUSIBLE candidate only with meaningful evidence:
/// at least this fraction of the winner's share (a clearly-not-red cell
/// never gets red as a candidate).
const PLAUSIBLE_FRACTION: f32 = 0.15;
/// How many low-margin cells may have their candidate set WIDENED to all
/// six colors when the evidence-constrained search finds nothing legal.
const MAX_FREED: usize = 6;
/// Search budget in visited NODES (not just completed assignments) — the
/// resolver must never freeze the UI thread.
const MAX_CHECKS: usize = 60_000;

/// Optimal class -> face assignment from the six CENTER histograms
/// (`centers[k][class]` = capture k's center share of `class`): the
/// permutation maximizing total evidence. Handles unreadable centers and
/// duplicate center votes, which greedy first-wins mapping cannot.
pub fn assign_classes(centers: &[[f32; 6]; 6]) -> [usize; 6] {
    // Brute-force over all 6! = 720 permutations: perm[k] = class of
    // capture k's center.
    let mut best = [0, 1, 2, 3, 4, 5];
    let mut best_score = f32::NEG_INFINITY;
    let mut perm = [0usize; 6];
    let mut used = [false; 6];
    fn rec(
        k: usize,
        centers: &[[f32; 6]; 6],
        perm: &mut [usize; 6],
        used: &mut [bool; 6],
        score: f32,
        best: &mut [usize; 6],
        best_score: &mut f32,
    ) {
        if k == 6 {
            if score > *best_score {
                *best_score = score;
                *best = *perm;
            }
            return;
        }
        for class in 0..6 {
            if used[class] {
                continue;
            }
            used[class] = true;
            perm[k] = class;
            rec(k + 1, centers, perm, used, score + centers[k][class], best, best_score);
            used[class] = false;
        }
    }
    rec(0, centers, &mut perm, &mut used, 0.0, &mut best, &mut best_score);
    best
}

/// Standard color scheme: which palette CLASS (0 w, 1 y, 2 r, 3 o, 4 g,
/// 5 b) each face carries on a standard cube (U white, D yellow, R red,
/// L orange, F green, B blue).
const STD_CLASS: [usize; 6] = [0, 2, 4, 1, 3, 5];

/// The scan labels faces by CAPTURE ORDER (first side shown = "F"), so a
/// red-first scan renders red as green. Given each capture-face's palette
/// class, rotate + relabel the cube so every color lands on its standard
/// face — the display then matches the physical cube, and so does the
/// guide's language. Returns None for cubes with a mirrored/nonstandard
/// scheme (keep the capture-order labeling there).
pub fn relabel_to_standard(
    cube: &FaceletCube,
    class_of_face: &[usize; 6],
) -> Option<FaceletCube> {
    for xz in ["", "x", "x2", "x'", "z", "z'"] {
        for y in ["", "y", "y2", "y'"] {
            let alg = cube_core::Alg::parse(&format!("{xz} {y}")).ok()?;
            let rotated = cube.applied_alg(&alg);
            // Does this rotation put every class on its standard face?
            let ok = (0..6).all(|f| {
                let old_label = rotated.0[f * 9 + 4] as usize;
                class_of_face[old_label] == STD_CLASS[f]
            });
            if !ok {
                continue;
            }
            // Relabel colors: old label -> the face it now sits on.
            let mut relabel = [Face::U; 6];
            for f in 0..6 {
                relabel[rotated.0[f * 9 + 4] as usize] = Face::from_index(f);
            }
            let mut out = rotated;
            for sticker in out.0.iter_mut() {
                *sticker = relabel[*sticker as usize];
            }
            return Some(out);
        }
    }
    None
}

pub fn resolve_scan(shares: &Shares) -> Result<FaceletCube, crate::ValidationError> {
    // Initial assignment: argmax per cell; centers are authoritative
    // (facelet 9f+4 is forced to its face by the scan flow).
    let ranked: Vec<Vec<Face>> = shares
        .iter()
        .map(|cell| {
            let mut order: Vec<usize> = (0..6).collect();
            order.sort_by(|&a, &b| cell[b].partial_cmp(&cell[a]).unwrap());
            order.into_iter().map(Face::from_index).collect()
        })
        .collect();
    let margin = |i: usize| -> f32 {
        let c = &shares[i];
        let mut v: Vec<f32> = c.to_vec();
        v.sort_by(|a, b| b.partial_cmp(a).unwrap());
        v[0] - v[1]
    };

    let mut base = FaceletCube::SOLVED;
    for i in 0..54 {
        base.0[i] = if i % 9 == 4 {
            Face::from_index(i / 9)
        } else {
            ranked[i][0]
        };
    }

    // EVERY non-center cell is searchable within its evidence-plausible
    // candidate width: clear cells have width 1 (de-facto fixed),
    // ambiguous ones 2-3, evidence-free ones all 6. The constraints pick
    // among plausible readings — never a color the evidence rules out.
    let widths: [usize; 54] = core::array::from_fn(|i| {
        let winner = shares[i][ranked[i][0] as usize];
        if winner < EVIDENCE_FLOOR {
            6 // no evidence: anything goes
        } else {
            let floor = (winner * PLAUSIBLE_FRACTION).max(0.02);
            ranked[i]
                .iter()
                .filter(|&&f| shares[i][f as usize] >= floor)
                .count()
                .max(1)
        }
    });

    // Widening fallback: when even the plausible readings admit no legal
    // cube, the evidence itself must be wrong somewhere. Cells whose
    // argmax color is OVERSUBSCRIBED (more than nine claimed) are the
    // prime suspects — widen those first (by margin), then everyone else.
    let mut counts = [0u8; 6];
    for i in 0..54 {
        counts[base.0[i] as usize] += 1;
    }
    let mut by_margin: Vec<usize> = (0..54).filter(|&i| i % 9 != 4).collect();
    by_margin.sort_by(|&a, &b| {
        let over_a = counts[base.0[a] as usize] > 9;
        let over_b = counts[base.0[b] as usize] > 9;
        over_b
            .cmp(&over_a)
            .then(margin(a).partial_cmp(&margin(b)).unwrap())
    });

    let mut checks = 0usize;
    for extra in 0..=MAX_FREED {
        let mut w = widths;
        for &i in by_margin.iter().take(extra) {
            w[i] = 6;
        }
        let mut free: Vec<usize> = (0..54).filter(|&i| i % 9 != 4 && w[i] > 1).collect();
        // MRV: fewest candidates first collapses the search tree.
        free.sort_by_key(|&i| w[i]);
        if let Some(found) = search(&base, &free, &ranked, &w, &mut checks) {
            return Ok(found);
        }
        if checks >= MAX_CHECKS {
            break;
        }
    }
    // Nothing legal found: hand back the argmax cube's specific error so
    // the review net can show it.
    Err(validate(&base).err().unwrap_or(crate::ValidationError::Unsolvable))
}

/// Depth-first over the free cells' ranked candidates with color-count
/// pruning; full validation only on complete assignments.
fn search(
    base: &FaceletCube,
    free: &[usize],
    ranked: &[Vec<Face>],
    widths: &[usize; 54],
    checks: &mut usize,
) -> Option<FaceletCube> {
    fn counts(state: &FaceletCube, skip: &[usize]) -> [u8; 6] {
        let mut c = [0u8; 6];
        for i in 0..54 {
            if !skip.contains(&i) {
                c[state.0[i] as usize] += 1;
            }
        }
        c
    }
    fn rec(
        state: &mut FaceletCube,
        free: &[usize],
        pos: usize,
        counts: &mut [u8; 6],
        ranked: &[Vec<Face>],
        widths: &[usize; 54],
        checks: &mut usize,
    ) -> Option<FaceletCube> {
        *checks += 1;
        if *checks >= MAX_CHECKS {
            return None;
        }
        if pos == free.len() {
            return validate(state).is_ok().then_some(*state);
        }
        let cell = free[pos];
        for &cand in ranked[cell].iter().take(widths[cell]) {
            if counts[cand as usize] >= 9 {
                continue; // color already fully placed
            }
            state.0[cell] = cand;
            counts[cand as usize] += 1;
            if let Some(found) = rec(state, free, pos + 1, counts, ranked, widths, checks) {
                return Some(found);
            }
            counts[cand as usize] -= 1;
        }
        None
    }

    let mut state = *base;
    let mut cnt = counts(base, free);
    rec(&mut state, free, 0, &mut cnt, ranked, widths, checks)
}

#[cfg(test)]
mod tests {
    use super::*;
    use cube_core::{Alg, SplitMix64};

    fn shares_from(state: &FaceletCube, hidden: &[usize], wrong: &[(usize, Face)]) -> Shares {
        let mut shares = [[0.0f32; 6]; 54];
        for i in 0..54 {
            shares[i][state.0[i] as usize] = 0.4; // solid evidence
        }
        for &h in hidden {
            shares[h] = [0.02; 6]; // no usable evidence
        }
        for &(i, f) in wrong {
            shares[i] = [0.0; 6];
            shares[i][f as usize] = 0.12; // confidently WRONG
            shares[i][state.0[i] as usize] = 0.08; // truth is runner-up
        }
        shares
    }

    #[test]
    fn fills_hidden_cells_to_a_legal_cube() {
        let mut rng = SplitMix64::new(11);
        for round in 0..30 {
            let mut state = FaceletCube::SOLVED;
            for _ in 0..15 {
                state.apply(cube_core::Move::from_index(rng.below(18) as usize));
            }
            // Hide a few random non-center cells.
            let mut hidden = Vec::new();
            while hidden.len() < 4 {
                let i = rng.below(54) as usize;
                if i % 9 != 4 && !hidden.contains(&i) {
                    hidden.push(i);
                }
            }
            let shares = shares_from(&state, &hidden, &[]);
            let resolved = resolve_scan(&shares)
                .unwrap_or_else(|e| panic!("round {round}: unresolvable: {e}"));
            assert!(crate::validate(&resolved).is_ok(), "round {round}");
            // With this few hidden cells the reconstruction is exact.
            assert_eq!(resolved, state, "round {round}: hidden {hidden:?}");
        }
    }

    #[test]
    fn recovers_from_a_confidently_wrong_cell() {
        let state = FaceletCube::SOLVED
            .applied_alg(&Alg::parse("R U F2 L' D B U2 R' F").unwrap());
        // Cell 7 (a U-face edge sticker) claims the wrong color with the
        // truth as runner-up: count/piece constraints must flip it back.
        let wrong_color = if state.0[7] == Face::R { Face::L } else { Face::R };
        let shares = shares_from(&state, &[], &[(7, wrong_color)]);
        let resolved = resolve_scan(&shares).expect("resolvable");
        assert_eq!(resolved, state, "constraints must correct the lie");
    }

    #[test]
    fn center_assignment_survives_collisions_and_blanks() {
        // Captures 1 and 5 both claim class 1; capture 3's center is
        // unreadable. The optimal assignment still gives each capture a
        // distinct class, preferring the stronger claims.
        let mut centers = [[0.0f32; 6]; 6];
        centers[0][2] = 0.30; // red, clear
        centers[1][1] = 0.28; // yellow, strong
        centers[2][3] = 0.25; // orange
        // capture 3: nothing readable
        centers[4][5] = 0.30; // blue
        centers[5][1] = 0.10; // ALSO claims yellow, weakly
        centers[5][4] = 0.06; // green as runner-up
        let assigned = assign_classes(&centers);
        // Distinct classes for all six captures.
        let mut sorted = assigned;
        sorted.sort_unstable();
        assert_eq!(sorted, [0, 1, 2, 3, 4, 5]);
        // The strong yellow keeps yellow; the weak one gets its runner-up.
        assert_eq!(assigned[1], 1);
        assert_eq!(assigned[5], 4);
        assert_eq!(assigned[0], 2);
        assert_eq!(assigned[4], 5);
    }

    #[test]
    fn relabel_puts_every_class_on_its_standard_face() {
        // Physical mapping from a real scan: capture-order faces carry
        // classes F=red R=green B=orange D=white L=blue U=yellow.
        let class_of_face = [1usize, 4, 2, 0, 5, 3]; // indexed U R F D L B
        // A scrambled cube in capture-order labels.
        let cube = FaceletCube::SOLVED
            .applied_alg(&Alg::parse("R U F2 L' D B U2 R' F D2 L").unwrap());
        let std = relabel_to_standard(&cube, &class_of_face).expect("standard scheme");
        // Centers canonical and each face now carries its standard class.
        for f in 0..6 {
            assert_eq!(std.0[f * 9 + 4] as usize, f, "center {f}");
        }
        assert!(crate::validate(&std).is_ok(), "relabeling must stay legal");
        // Identity mapping: nothing to do, cube unchanged.
        let identity = [0usize, 2, 4, 1, 3, 5];
        assert_eq!(relabel_to_standard(&cube, &identity), Some(cube));
    }

    #[test]
    fn hopeless_evidence_reports_a_validation_error() {
        // All cells claim white: no legal cube exists near this evidence.
        let mut shares = [[0.0f32; 6]; 54];
        for cell in shares.iter_mut() {
            cell[0] = 0.5;
        }
        assert!(resolve_scan(&shares).is_err());
    }
}
