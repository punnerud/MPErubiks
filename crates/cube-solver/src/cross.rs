//! Micro-solver for the first cross: IDA* over the four cross edges.
//!
//! The cross is the one stage that cannot be macro'd (≈190k configs, no
//! named algorithms) — the "Min vei" macro-router needs it solved before
//! the algorithm graph takes over. State = the four D-color edges'
//! (slot, orientation); heuristic = max over the four edges of that
//! edge's exact single-edge solve distance (precomputed BFS, admissible).
//! Any cross is solvable in <= 8 moves; typical searches visit a few
//! thousand nodes.

use cube_core::{Alg, Face, FaceletCube, Move};

/// Solve the cross of the DOWN face's color (by centers). Returns the
/// shortest face-move sequence (<= 9 as a safety bound), or None if the
/// state is malformed.
pub fn solve_cross(state: &FaceletCube) -> Option<Alg> {
    let down = state.center(Face::D);
    // The four cross edges, identified by their color pairs.
    let sides = [Face::F, Face::R, Face::B, Face::L].map(|f| state.center(f));
    let targets: [[Face; 2]; 4] = core::array::from_fn(|i| [down, sides[i]]);

    // Home (slot, ori) per cross edge, IN THE SAME COLOR-ORDER as the
    // lookups below (slots/oris are positional, frame-independent).
    let homes: [(u8, u8); 4] = core::array::from_fn(|i| {
        FaceletCube::SOLVED
            .locate_edge([Face::D, [Face::F, Face::R, Face::B, Face::L][i]])
            .expect("solved cube has all edges")
    });
    let solved = |s: &FaceletCube| -> bool {
        targets
            .iter()
            .enumerate()
            .all(|(i, pair)| s.locate_edge(*pair) == Some(homes[i]))
    };
    if solved(state) {
        return Some(Alg::new(Vec::new()));
    }

    // Exact per-edge distances: BFS over the 24 (slot, ori) states of a
    // single cross edge, per target slot.
    let dist = single_edge_distances(&homes);
    let h = |s: &FaceletCube| -> usize {
        let mut worst = 0;
        for (i, pair) in targets.iter().enumerate() {
            let Some((slot, ori)) = s.locate_edge(*pair) else {
                return usize::MAX;
            };
            worst = worst.max(dist[i][slot as usize * 2 + ori as usize] as usize);
        }
        worst
    };

    fn dfs(
        s: &FaceletCube,
        depth_left: usize,
        last_face: Option<u8>,
        path: &mut Vec<Move>,
        solved: &dyn Fn(&FaceletCube) -> bool,
        h: &dyn Fn(&FaceletCube) -> usize,
    ) -> bool {
        if solved(s) {
            return true;
        }
        let est = h(s);
        if est > depth_left {
            return false;
        }
        for idx in 0..18 {
            let face = idx as u8 / 3;
            if last_face == Some(face) {
                continue;
            }
            let mv = Move::from_index(idx);
            let mut next = *s;
            next.apply(mv);
            path.push(mv);
            if dfs(&next, depth_left - 1, Some(face), path, solved, h) {
                return true;
            }
            path.pop();
        }
        false
    }

    for bound in 1..=9usize {
        let mut path = Vec::new();
        if dfs(state, bound, None, &mut path, &solved, &h) {
            return Some(Alg::new(path));
        }
    }
    None
}

/// dist[target][slot*2+ori] = exact moves to bring a lone cross edge
/// from (slot, ori) home to `target` (BFS from the goal, breadth 18).
fn single_edge_distances(homes: &[(u8, u8); 4]) -> [[u8; 24]; 4] {
    core::array::from_fn(|i| {
        let goal = homes[i].0 as usize * 2 + homes[i].1 as usize;
        let mut dist = [u8::MAX; 24];
        dist[goal] = 0;
        // BFS in the quotient space of ONE edge: simulate by moving a
        // marked edge on an otherwise ignored cube.
        let mut frontier = vec![goal];
        // Track states as (slot, ori); transition via a probe cube where
        // the marked edge starts at (slot, ori).
        while let Some(cur) = frontier.pop() {
            let d = dist[cur];
            for idx in 0..18 {
                let mv = Move::from_index(idx);
                let next = edge_transition(cur, mv);
                if dist[next] > d + 1 {
                    dist[next] = d + 1;
                    frontier.push(next);
                }
            }
        }
        dist
    })
}

/// Where does the edge at (slot, ori) = code/2, code%2 go under `mv`?
/// Probed on a real cube: place the UF edge pattern at the coded slot.
fn edge_transition(code: usize, mv: Move) -> usize {
    static TABLE: std::sync::OnceLock<[[u8; 24]; 18]> = std::sync::OnceLock::new();
    let table = TABLE.get_or_init(|| {
        // For every (slot, ori), find a placement by searching a probe:
        // take SOLVED, locate the (U,F) edge as marker after moving it
        // around is complex — instead probe transitions by applying mv to
        // solved-cube edges: for each edge e of the 12, and each ori, the
        // (slot, ori) -> (slot', ori') map is the same for ALL edges
        // (moves permute slots uniformly). Use edge (U,F) tracked through
        // single moves from every reachable position via BFS from home.
        let mut map = [[u8::MAX; 24]; 18];
        // Seed: home state of (U,F).
        let home = {
            let s = FaceletCube::SOLVED;
            let (slot, ori) = s.locate_edge([Face::U, Face::F]).unwrap();
            (slot as usize) * 2 + ori as usize
        };
        // BFS over cube states restricted to tracking one edge is not
        // possible state-locally; instead walk cubes breadth-first until
        // every (slot, ori) has been SEEN as the marker's position, and
        // record transitions from each seen cube.
        let mut seen_codes = [false; 24];
        let mut queue = std::collections::VecDeque::new();
        queue.push_back(FaceletCube::SOLVED);
        seen_codes[home] = true;
        let mut guard = 0;
        while let Some(cube) = queue.pop_front() {
            guard += 1;
            if guard > 3000 {
                break;
            }
            let (slot, ori) = cube.locate_edge([Face::U, Face::F]).unwrap();
            let code = (slot as usize) * 2 + ori as usize;
            let mut all_known = true;
            for idx in 0..18 {
                if map[idx][code] == u8::MAX {
                    all_known = false;
                    let mut n = cube;
                    n.apply(Move::from_index(idx));
                    let (s2, o2) = n.locate_edge([Face::U, Face::F]).unwrap();
                    let code2 = (s2 as usize) * 2 + o2 as usize;
                    map[idx][code] = code2 as u8;
                    if !seen_codes[code2] {
                        seen_codes[code2] = true;
                        queue.push_back(n);
                    }
                }
            }
            if !all_known {
                // Re-enqueue neighbors already added above.
            }
        }
        map
    });
    let idx = match mv {
        m => {
            // Move::from_index inverse: find the index.
            (0..18)
                .find(|&i| Move::from_index(i) == m)
                .expect("face move")
        }
    };
    table[idx][code] as usize
}

#[cfg(test)]
mod tests {
    use super::*;
    use cube_core::SplitMix64;

    #[test]
    fn cross_solves_random_scrambles_short() {
        let mut rng = SplitMix64::new(9);
        for round in 0..40 {
            let mut s = FaceletCube::SOLVED;
            for _ in 0..20 {
                s.apply(Move::from_index(rng.below(18) as usize));
            }
            let alg = solve_cross(&s).unwrap_or_else(|| panic!("round {round}: no cross"));
            assert!(alg.0.len() <= 9, "round {round}: {} moves", alg.0.len());
            let done = s.applied_alg(&alg);
            // All four cross edges home, oriented.
            for f in [Face::F, Face::R, Face::B, Face::L] {
                let pair = [done.center(Face::D), done.center(f)];
                let (slot, ori) = done.locate_edge(pair).unwrap();
                let home = FaceletCube::SOLVED.locate_edge([Face::D, f]).unwrap();
                assert_eq!((slot, ori), home, "round {round}: {f:?} edge");
            }
        }
    }
}
