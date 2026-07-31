//! Case recognition for algorithm training and hints.
//!
//! Each case is defined ONLY by its algorithm: the recognition pattern is
//! derived at load time by applying the algorithm's inverse to a solved
//! cube. Bad data in the algorithm library therefore fails loudly here
//! (pattern collisions, wrong-stage algorithms) instead of silently
//! misrecognizing.
//!
//! Matching is invariant to AUF (U-face pre-rotation) — all four U-shifted
//! patterns are pre-inserted — and, where it applies, to whole-cube y
//! rotation (PLL via a face-relative key, F2L by probing the four y frames).

use crate::facelet::{face_of, Face, FaceletCube};
use crate::moves::{Alg, Move, Turns};
use crate::rng::SplitMix64;
use std::collections::HashMap;
use std::fmt;

#[derive(Clone, Copy, PartialEq, Eq, Hash, Debug)]
pub enum CaseSet {
    Pll,
    Oll,
    F2l,
    Lbl,
}

#[derive(Clone, Copy, PartialEq, Eq, Debug)]
pub enum RecogKind {
    Oll,
    Pll,
    F2l,
    /// Playback/drill only — no automatic recognition (some LBL steps).
    None,
}

#[derive(Clone, Debug)]
pub struct CaseDef {
    /// Stable id, e.g. "pll-t".
    pub id: String,
    pub set: CaseSet,
    /// Human name, e.g. "T-Perm".
    pub name: String,
    /// Grouping label for pickers, e.g. "adjacent swap".
    pub group: String,
    pub alg: Alg,
    pub recognition: RecogKind,
}

#[derive(Clone, Debug)]
pub enum LibraryError {
    /// The algorithm does not produce a state of the claimed kind
    /// (e.g. an "OLL" alg that scrambles F2L).
    BadAlg { id: String, reason: String },
    /// Two different cases produce the same recognition pattern.
    Collision { a: String, b: String },
}

impl fmt::Display for LibraryError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            LibraryError::BadAlg { id, reason } => write!(f, "bad algorithm '{id}': {reason}"),
            LibraryError::Collision { a, b } => {
                write!(f, "cases '{a}' and '{b}' are indistinguishable")
            }
        }
    }
}

impl std::error::Error for LibraryError {}

/// A recognized case at a concrete state.
#[derive(Clone, Copy, PartialEq, Eq, Debug)]
pub struct Match {
    /// Index into the `Recognizer`'s case list.
    pub case_idx: u16,
    /// Number of clockwise U quarter-turns to perform before the algorithm.
    pub pre_auf: u8,
    /// Whole-cube y frame the case was seen in (0..4). Execute with
    /// `Recognizer::execution_alg`, which folds this in.
    pub y_frame: u8,
}

/// The 21 last-layer sticker positions: U face, then the top rows of
/// F, R, B, L.
const LL21: [usize; 21] = [
    0, 1, 2, 3, 4, 5, 6, 7, 8, // U1..U9
    18, 19, 20, // F1..F3
    9, 10, 11, // R1..R3
    45, 46, 47, // B1..B3
    36, 37, 38, // L1..L3
];

/// Side order following the y cycle (F→L→B→R): used for the y-invariant
/// PLL key.
fn side_idx(f: Face) -> Option<u32> {
    match f {
        Face::F => Some(0),
        Face::L => Some(1),
        Face::B => Some(2),
        Face::R => Some(3),
        _ => None,
    }
}

fn oll_pattern(s: &FaceletCube) -> u32 {
    let u = s.center(Face::U);
    let mut bits = 0u32;
    for (k, &i) in LL21.iter().enumerate() {
        if s.0[i] == u {
            bits |= 1 << k;
        }
    }
    bits
}

/// 12 side stickers of the last layer as 2-bit digits, each the color's
/// side index relative to the face it sits on. Invariant under whole-cube
/// y rotation. `None` if any sticker is not a side color (i.e. OLL not
/// actually done).
fn pll_key(s: &FaceletCube) -> Option<u32> {
    let mut key = 0u32;
    for (k, &i) in LL21[9..].iter().enumerate() {
        let color = side_idx(s.0[i])?;
        let face = side_idx(face_of(i)).expect("LL side positions are on side faces");
        let digit = (color + 4 - face) % 4;
        key |= digit << (2 * k);
    }
    Some(key)
}

/// Piece-level key for the FR-slot F2L pair in the given (already
/// y-normalized) state: locations and orientations of the corner
/// {D,F,R-colors} and edge {F,R-colors}, read relative to centers.
fn f2l_key(s: &FaceletCube) -> Option<u16> {
    let d = s.center(Face::D);
    let f = s.center(Face::F);
    let r = s.center(Face::R);
    let (cs, co) = s.locate_corner([d, f, r])?;
    let (es, eo) = s.locate_edge([f, r])?;
    Some(u16::from(cs) | (u16::from(co) << 3) | (u16::from(es) << 5) | (u16::from(eo) << 9))
}

pub struct Recognizer {
    defs: Vec<CaseDef>,
    /// Per case: y-frame correction for execution. An algorithm with a net
    /// whole-cube y rotation (wide/slice moves that don't cancel) solves its
    /// case in a rotated frame; canonical states are center-normalized, so
    /// the alg must be conjugated back by this amount when executed.
    frame_fix: Vec<u8>,
    oll: HashMap<u32, (u16, u8)>,
    pll: HashMap<u32, (u16, u8)>,
    f2l: HashMap<u16, (u16, u8)>,
}

/// Net whole-cube rotation of the algorithm, if it is a pure y rotation:
/// the number of y quarter-turns the cube ends up rotated by. `None` when
/// the net rotation tilts the cube (U/D leave the vertical axis).
fn net_y_rotation(alg: &Alg) -> Option<u8> {
    let t = FaceletCube::SOLVED.applied_alg(alg);
    if t.center(Face::U) != Face::U || t.center(Face::D) != Face::D {
        return None;
    }
    // Under y^1, F content shows at L.
    for (k, f) in [Face::F, Face::L, Face::B, Face::R].into_iter().enumerate() {
        if t.center(f) == Face::F {
            return Some(k as u8);
        }
    }
    None
}

impl Recognizer {
    pub fn new(library: Vec<CaseDef>) -> Result<Recognizer, LibraryError> {
        let mut rec = Recognizer {
            defs: Vec::new(),
            frame_fix: Vec::new(),
            oll: HashMap::new(),
            pll: HashMap::new(),
            f2l: HashMap::new(),
        };
        for def in library {
            rec.insert(def)?;
        }
        Ok(rec)
    }

    pub fn defs(&self) -> &[CaseDef] {
        &self.defs
    }

    pub fn case(&self, idx: u16) -> &CaseDef {
        &self.defs[idx as usize]
    }

    pub fn find_by_id(&self, id: &str) -> Option<u16> {
        self.defs.iter().position(|d| d.id == id).map(|i| i as u16)
    }

    fn insert(&mut self, def: CaseDef) -> Result<(), LibraryError> {
        let idx = self.defs.len() as u16;
        let bad = |reason: &str| LibraryError::BadAlg {
            id: def.id.clone(),
            reason: reason.into(),
        };

        let Some(net_y) = net_y_rotation(&def.alg) else {
            return Err(bad(
                "net whole-cube rotation tilts the cube — append rotations (x/z) so only a y rotation remains",
            ));
        };
        let frame_fix = (4 - net_y) % 4;

        // A case is a class of 4x4 states: the solve may end with any final
        // AUF (`a`, applied before the inverse when generating) and the
        // state may be seen with any pre-AUF (`k`). For permutation cases
        // (PLL) the `a` side genuinely produces distinct keys; for
        // orientation- and piece-based keys (OLL, F2L) it collapses into
        // the same entries, which `insert_key` merges.
        for a in 0..4u8 {
            let canonical = FaceletCube::SOLVED
                .rotate_u(a)
                .applied_alg(&def.alg.inverse())
                .normalize_orientation();

            match def.recognition {
                RecogKind::Oll => {
                    if !canonical.is_f2l_solved() {
                        return Err(bad("inverse does not preserve F2L — not an OLL alg"));
                    }
                    if canonical.is_oll_done() {
                        return Err(bad("inverse leaves U face oriented — nothing to recognize"));
                    }
                    let mut state = canonical;
                    for k in 0..4u8 {
                        let entry = (idx, (4 - k) % 4);
                        insert_key(&mut self.oll, oll_pattern(&state), entry, &self.defs, &def)?;
                        state.apply(Move::Face(Face::U, Turns::Cw));
                    }
                }
                RecogKind::Pll => {
                    if !canonical.is_f2l_solved() || !canonical.is_oll_done() {
                        return Err(bad("inverse does not preserve F2L+OLL — not a PLL alg"));
                    }
                    if canonical.is_solved() {
                        return Err(bad("inverse leaves the cube solved — nothing to recognize"));
                    }
                    let mut state = canonical;
                    for k in 0..4u8 {
                        let key = pll_key(&state).ok_or_else(|| bad("PLL key failed"))?;
                        insert_key(&mut self.pll, key, (idx, (4 - k) % 4), &self.defs, &def)?;
                        state.apply(Move::Face(Face::U, Turns::Cw));
                    }
                }
                RecogKind::F2l => {
                    if !canonical.is_f2l_minus_fr_solved() {
                        return Err(bad(
                            "inverse disturbs more than the FR pair and last layer — not an FR-slot F2L alg",
                        ));
                    }
                    let mut state = canonical;
                    for k in 0..4u8 {
                        let key = f2l_key(&state).ok_or_else(|| bad("F2L pieces not found"))?;
                        insert_key(&mut self.f2l, key, (idx, (4 - k) % 4), &self.defs, &def)?;
                        state.apply(Move::Face(Face::U, Turns::Cw));
                    }
                }
                RecogKind::None => {}
            }
        }

        self.defs.push(def);
        self.frame_fix.push(frame_fix);
        Ok(())
    }

    /// Recognize the state against the given sets. Stage preconditions are
    /// enforced (OLL needs F2L solved, PLL needs OLL done, F2L needs the
    /// other three slots and cross intact in the matched frame).
    pub fn recognize(&self, s: &FaceletCube, sets: &[CaseSet]) -> Vec<Match> {
        let mut out = Vec::new();
        let f2l_done = s.is_f2l_solved();
        let oll_done = s.is_oll_done();

        let want = |case_idx: u16| sets.contains(&self.defs[case_idx as usize].set);

        if f2l_done && !oll_done {
            if let Some(&(case_idx, pre_auf)) = self.oll.get(&oll_pattern(s)) {
                if want(case_idx) {
                    out.push(Match {
                        case_idx,
                        pre_auf,
                        y_frame: 0,
                    });
                }
            }
        }
        if f2l_done && oll_done && !s.is_solved() {
            if let Some(&(case_idx, pre_auf)) = pll_key(s).and_then(|k| self.pll.get(&k)) {
                if want(case_idx) {
                    out.push(Match {
                        case_idx,
                        pre_auf,
                        y_frame: 0,
                    });
                }
            }
        }
        if !f2l_done {
            for j in 0..4u8 {
                let sj = s.rotate_y(j);
                if !sj.is_f2l_minus_fr_solved() {
                    continue;
                }
                if let Some(&(case_idx, pre_auf)) = f2l_key(&sj).and_then(|k| self.f2l.get(&k)) {
                    if want(case_idx) {
                        out.push(Match {
                            case_idx,
                            pre_auf,
                            y_frame: j,
                        });
                    }
                }
            }
        }
        out
    }

    /// Does this specific case match the state? (Used by the trainer.)
    pub fn recognize_case(&self, s: &FaceletCube, case_idx: u16) -> Option<Match> {
        let def = &self.defs[case_idx as usize];
        match def.recognition {
            RecogKind::Oll => match self.oll.get(&oll_pattern(s)) {
                Some(&(idx, pre_auf)) if idx == case_idx => Some(Match {
                    case_idx,
                    pre_auf,
                    y_frame: 0,
                }),
                _ => None,
            },
            RecogKind::Pll => match pll_key(s).and_then(|k| self.pll.get(&k)) {
                Some(&(idx, pre_auf)) if idx == case_idx => Some(Match {
                    case_idx,
                    pre_auf,
                    y_frame: 0,
                }),
                _ => None,
            },
            RecogKind::F2l => {
                for j in 0..4u8 {
                    let sj = s.rotate_y(j);
                    if let Some(&(idx, pre_auf)) = f2l_key(&sj).and_then(|k| self.f2l.get(&k)) {
                        if idx == case_idx {
                            return Some(Match {
                                case_idx,
                                pre_auf,
                                y_frame: j,
                            });
                        }
                    }
                }
                None
            }
            RecogKind::None => None,
        }
    }

    /// The moves to physically perform for a match: optional U pre-turn,
    /// then the case algorithm (frame-corrected for any net y rotation of
    /// the alg itself), relabeled into the matched y frame.
    pub fn execution_alg(&self, m: Match) -> Alg {
        let def = &self.defs[m.case_idx as usize];
        let corrected = def.alg.in_y_frame(self.frame_fix[m.case_idx as usize]);
        let mut moves = Vec::with_capacity(corrected.0.len() + 1);
        match m.pre_auf % 4 {
            1 => moves.push(Move::Face(Face::U, Turns::Cw)),
            2 => moves.push(Move::Face(Face::U, Turns::Half)),
            3 => moves.push(Move::Face(Face::U, Turns::Ccw)),
            _ => {}
        }
        moves.extend(corrected.0);
        Alg::new(moves).in_y_frame(m.y_frame)
    }

    /// The case's canonical state (inverse alg on solved, centers home) —
    /// what diagrams should show.
    pub fn canonical_state(&self, case_idx: u16) -> FaceletCube {
        let def = &self.defs[case_idx as usize];
        FaceletCube::SOLVED
            .applied_alg(&def.alg.inverse())
            .normalize_orientation()
    }

    /// A practice state for the case: solved (up to a random AUF), with the
    /// case's inverse applied and a random pre-AUF on top.
    pub fn setup_state(&self, case_idx: u16, rng: &mut SplitMix64) -> FaceletCube {
        let def = &self.defs[case_idx as usize];
        let mut s = FaceletCube::SOLVED;
        for _ in 0..rng.below(4) {
            s.apply(Move::Face(Face::U, Turns::Cw));
        }
        s.apply_alg(&def.alg.inverse());
        for _ in 0..rng.below(4) {
            s.apply(Move::Face(Face::U, Turns::Cw));
        }
        s.normalize_orientation()
    }
}

fn insert_key<K: std::hash::Hash + Eq + Copy>(
    map: &mut HashMap<K, (u16, u8)>,
    key: K,
    entry: (u16, u8),
    defs: &[CaseDef],
    inserting: &CaseDef,
) -> Result<(), LibraryError> {
    match map.get_mut(&key) {
        None => {
            map.insert(key, entry);
            Ok(())
        }
        Some(existing) if existing.0 == entry.0 => {
            // Same case seen from a symmetric AUF: keep the smallest pre-AUF.
            if entry.1 < existing.1 {
                existing.1 = entry.1;
            }
            Ok(())
        }
        Some(existing) => Err(LibraryError::Collision {
            a: defs[existing.0 as usize].id.clone(),
            b: inserting.id.clone(),
        }),
    }
}
