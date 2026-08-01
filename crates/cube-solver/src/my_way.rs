//! «Min vei» — the macro-router: MPEE applied to the algorithm graph.
//!
//! The plan is a sequence of ALGORITHM IDs — longer in moves than kewb's
//! answer, but executed from muscle memory: predicted human time is the
//! objective, and every segment is one of the algorithms the lessons
//! teach. Engine: greedy lexicographic progress over a candidate
//! vocabulary (each beginner algorithm × 4 y-frames × 4 pre-AUF
//! alignments × small repetition counts), where the progress vector
//! rewards both direct progress and EXTRACTIONS (freeing a buried piece
//! to the top layer counts). Cross is micro-solved (IDA*); anything the
//! vocabulary cannot improve falls to a kewb tail — honest, never stuck.

use crate::cross::solve_cross;
use cube_core::{Alg, Face, FaceletCube, Move, Recognizer, Turns};

#[derive(Clone, Debug)]
pub struct MySeg {
    /// The named algorithm, or None for raw segments (x2, cross, AUF, tail).
    pub case_idx: Option<u16>,
    /// Leading alignment moves (pre-AUF) of `alg` — shown ungated.
    pub auf_len: u8,
    pub alg: Alg,
    pub est_ms: u32,
}

#[derive(Clone, Debug)]
pub struct MyWayPlan {
    pub segments: Vec<MySeg>,
    pub total_ms: u32,
}

fn default_est(alg: &Alg) -> u32 {
    700 + 300 * alg.len_htm() as u32
}

fn all_faces_uniform(s: &FaceletCube) -> bool {
    (0..6).all(|f| {
        let c = s.0[f * 9 + 4];
        (0..9).all(|o| s.0[f * 9 + o] == c)
    })
}

/// Lexicographic progress through the beginner method, all stages at
/// once (later stages only matter once earlier ones are full):
/// cross, D-corners (+extractable bonus), E-edges (+extractable bonus),
/// top cross, top pieces placed, top corners oriented, fully solved.
fn progress(s: &FaceletCube) -> [u8; 9] {
    let d = s.center(Face::D);
    let u = s.center(Face::U);
    let sides = [Face::F, Face::R, Face::B, Face::L].map(|f| s.center(f));
    let solved = FaceletCube::SOLVED;

    let mut cross = 0;
    let mut d_corners = 0;
    let mut d_corners_top = 0;
    let mut e_edges = 0;
    let mut e_edges_top = 0;
    let mut top_cross = 0;
    let mut top_edges = 0;
    let mut top_corners_pos = 0;
    let mut top_corners_ori = 0;

    for i in 0..4 {
        let f = sides[i];
        let fpos = [Face::F, Face::R, Face::B, Face::L][i];
        if s.locate_edge([d, f]) == solved.locate_edge([Face::D, fpos]) {
            cross += 1;
        }
        let g = sides[(i + 1) % 4];
        let gpos = [Face::F, Face::R, Face::B, Face::L][(i + 1) % 4];
        if let (Some(loc), Some(home)) = (
            s.locate_corner([d, f, g]),
            solved.locate_corner([Face::D, fpos, gpos]),
        ) {
            if loc == home {
                d_corners += 1;
            } else if corner_in_top(s, [d, f, g]) {
                d_corners_top += 1;
            }
        }
        if s.locate_edge([f, g]) == solved.locate_edge([fpos, gpos]) {
            e_edges += 1;
        } else if edge_in_top(s, [f, g]) {
            e_edges_top += 1;
        }
        if s.locate_edge([u, f]) == solved.locate_edge([Face::U, fpos]) {
            top_edges += 1;
        }
        if let (Some((slot, ori)), Some((hslot, hori))) = (
            s.locate_corner([u, f, g]),
            solved.locate_corner([Face::U, fpos, gpos]),
        ) {
            if slot == hslot {
                top_corners_pos += 1;
                if ori == hori {
                    top_corners_ori += 1;
                }
            }
        }
    }
    for o in [1usize, 3, 5, 7] {
        if s.0[Face::U as usize * 9 + o] == u {
            top_cross += 1;
        }
    }
    [
        cross,
        d_corners,
        d_corners_top,
        e_edges,
        e_edges_top,
        top_cross,
        top_edges + top_corners_pos,
        top_corners_ori,
        u8::from(all_faces_uniform(s)),
    ]
}

/// Is this corner physically in the top layer?
fn corner_in_top(s: &FaceletCube, colors: [Face; 3]) -> bool {
    for o in [0usize, 2, 6, 8] {
        if corner_colors_at_u(s, o)
            .is_some_and(|set| colors.iter().all(|c| set.contains(c)))
        {
            return true;
        }
    }
    false
}

/// The three sticker colors of the corner at U-face offset o (Kociemba:
/// U0:L0+B2 · U2:B0+R2 · U6:F0+L2 · U8:R0+F2).
fn corner_colors_at_u(s: &FaceletCube, o: usize) -> Option<[Face; 3]> {
    let (a, b) = match o {
        0 => ((Face::L, 0), (Face::B, 2)),
        2 => ((Face::B, 0), (Face::R, 2)),
        6 => ((Face::F, 0), (Face::L, 2)),
        8 => ((Face::R, 0), (Face::F, 2)),
        _ => return None,
    };
    Some([
        s.0[Face::U as usize * 9 + o],
        s.0[a.0 as usize * 9 + a.1],
        s.0[b.0 as usize * 9 + b.1],
    ])
}

/// Is this edge physically in the top layer? (U1:B1 · U3:L1 · U5:R1 · U7:F1)
fn edge_in_top(s: &FaceletCube, colors: [Face; 2]) -> bool {
    for (o, side) in [(1usize, Face::B), (3, Face::L), (5, Face::R), (7, Face::F)] {
        let pair = [s.0[Face::U as usize * 9 + o], s.0[side as usize * 9 + 1]];
        if (pair[0] == colors[0] && pair[1] == colors[1])
            || (pair[0] == colors[1] && pair[1] == colors[0])
        {
            return true;
        }
    }
    false
}

struct Candidate {
    case_idx: Option<u16>,
    auf: u8,
    alg: Alg,
    est_ms: u32,
}

/// Build the full macro plan. `cost` = the user's measured average
/// execution time per case in ms (None = unmeasured).
pub fn my_way(
    state: &FaceletCube,
    rec: &Recognizer,
    cost: &dyn Fn(u16) -> Option<u32>,
) -> Option<MyWayPlan> {
    if all_faces_uniform(state) {
        return Some(MyWayPlan {
            segments: Vec::new(),
            total_ms: 0,
        });
    }
    let mut segments: Vec<MySeg> = Vec::new();
    let mut total: u32 = 0;

    // 1. White down.
    let x2 = Alg::parse("x2").ok()?;
    let mut work = state.applied_alg(&x2);
    segments.push(MySeg {
        case_idx: None,
        auf_len: 0,
        alg: x2,
        est_ms: 900,
    });
    total += 900;

    // 2. Cross (micro-IDA*).
    let cross = solve_cross(&work)?;
    if !cross.0.is_empty() {
        work = work.applied_alg(&cross);
        let est = 500 + 350 * cross.0.len() as u32;
        segments.push(MySeg {
            case_idx: None,
            auf_len: 0,
            alg: cross,
            est_ms: est,
        });
        total += est;
    }

    // 3. Greedy lexicographic router over the lesson vocabulary.
    let vocab: Vec<(u16, u8)> = [
        ("lbl-corner-insert", 5u8),
        ("lbl-second-layer-right", 1),
        ("lbl-second-layer-left", 1),
        ("lbl-top-cross", 1),
        ("lbl-sune", 2),
        ("lbl-edge-cycle", 2),
        ("lbl-corner-cycle", 2),
    ]
    .into_iter()
    .filter_map(|(id, reps)| rec.find_by_id(id).map(|idx| (idx, reps)))
    .collect();

    let mut guard = 0;
    while guard < 40 && !all_faces_uniform(&work) {
        guard += 1;
        let cur = progress(&work);
        let mut best: Option<(Candidate, [u8; 9], FaceletCube)> = None;
        let mut consider =
            |cand: Candidate, best: &mut Option<(Candidate, [u8; 9], FaceletCube)>| {
                let next = work.applied_alg(&cand.alg);
                let p = progress(&next);
                if p <= cur {
                    return;
                }
                let better = match best {
                    None => true,
                    Some((b, bp, _)) => p > *bp || (p == *bp && cand.est_ms < b.est_ms),
                };
                if better {
                    *best = Some((cand, p, next));
                }
            };
        for &(case_idx, max_reps) in &vocab {
            let base = rec.case(case_idx).alg.clone();
            for frame in 0..4u8 {
                let framed = base.in_y_frame(frame).face_moves_only();
                let base_est = cost(case_idx).unwrap_or_else(|| default_est(&framed));
                for auf in 0..4u8 {
                    for reps in 1..=max_reps {
                        let mut moves: Vec<Move> = Vec::new();
                        match auf {
                            1 => moves.push(Move::Face(Face::U, Turns::Cw)),
                            2 => moves.push(Move::Face(Face::U, Turns::Half)),
                            3 => moves.push(Move::Face(Face::U, Turns::Ccw)),
                            _ => {}
                        }
                        for _ in 0..reps {
                            moves.extend(framed.0.iter().copied());
                        }
                        consider(
                            Candidate {
                                case_idx: Some(case_idx),
                                auf: u8::from(auf != 0),
                                alg: Alg::new(moves),
                                est_ms: base_est * u32::from(reps)
                                    + 150 * u32::from(auf != 0),
                            },
                            &mut best,
                        );
                    }
                }
            }
        }
        // Pure AUF (final alignment).
        for auf in 1..4u8 {
            let turns = match auf {
                1 => Turns::Cw,
                2 => Turns::Half,
                _ => Turns::Ccw,
            };
            consider(
                Candidate {
                    case_idx: None,
                    auf: 0,
                    alg: Alg::new(vec![Move::Face(Face::U, turns)]),
                    est_ms: 400,
                },
                &mut best,
            );
        }

        match best {
            Some((cand, _, next)) => {
                work = next;
                total += cand.est_ms;
                segments.push(MySeg {
                    case_idx: cand.case_idx,
                    auf_len: cand.auf,
                    alg: cand.alg,
                    est_ms: cand.est_ms,
                });
            }
            None => break, // vocabulary exhausted: tail below
        }
    }

    // 4. Honest tail (kewb, conjugated into the x2 frame: U<->D, F<->B).
    if !all_faces_uniform(&work) {
        let normalized = work.normalize_orientation();
        let tail = crate::solve(&normalized).ok()?;
        let conj = |m: Move| -> Move {
            match m {
                Move::Face(f, t) => Move::Face(
                    match f {
                        Face::U => Face::D,
                        Face::D => Face::U,
                        Face::F => Face::B,
                        Face::B => Face::F,
                        other => other,
                    },
                    t,
                ),
                other => other,
            }
        };
        let tail = Alg::new(tail.0.iter().map(|&m| conj(m)).collect());
        let est = 900 + 350 * tail.0.len() as u32;
        work = work.applied_alg(&tail);
        segments.push(MySeg {
            case_idx: None,
            auf_len: 0,
            alg: tail,
            est_ms: est,
        });
        total += est;
    }

    all_faces_uniform(&work).then_some(MyWayPlan {
        segments,
        total_ms: total,
    })
}
