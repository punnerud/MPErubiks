//! Move notation: parsing, display, inversion, frame conjugation and
//! reduction of wide/slice/rotation moves to face moves.
//!
//! Internally every move maps to a canonical `(Dir, LayerKind, Turns)`
//! triple, where `Dir` is an oriented axis. "Cw" always means clockwise as
//! seen from outside the cube looking along `-dir`, which for an oriented
//! axis `d` is a `-90°` right-hand rotation about `d`.

use std::fmt;

#[derive(Clone, Copy, PartialEq, Eq, Hash, Debug)]
pub enum Turns {
    Cw,
    Half,
    Ccw,
}

impl Turns {
    pub fn inverse(self) -> Turns {
        match self {
            Turns::Cw => Turns::Ccw,
            Turns::Half => Turns::Half,
            Turns::Ccw => Turns::Cw,
        }
    }

    pub fn quarters(self) -> u8 {
        match self {
            Turns::Cw => 1,
            Turns::Half => 2,
            Turns::Ccw => 3,
        }
    }

    fn suffix(self) -> &'static str {
        match self {
            Turns::Cw => "",
            Turns::Half => "2",
            Turns::Ccw => "'",
        }
    }
}

#[derive(Clone, Copy, PartialEq, Eq, Hash, Debug)]
pub enum SliceKind {
    M,
    E,
    S,
}

#[derive(Clone, Copy, PartialEq, Eq, Hash, Debug)]
pub enum RotAxis {
    X,
    Y,
    Z,
}

#[derive(Clone, Copy, PartialEq, Eq, Hash, Debug)]
pub enum Move {
    /// Outer face turn: U R F D L B.
    Face(crate::Face, Turns),
    /// Wide (two-layer) turn: Uw/u, Rw/r, ...
    Wide(crate::Face, Turns),
    /// Middle slice: M (follows L), E (follows D), S (follows F).
    Slice(SliceKind, Turns),
    /// Whole-cube rotation: x (like R), y (like U), z (like F).
    Rot(RotAxis, Turns),
}

/// Oriented axis: `sign * e_axis` with axis 0=X, 1=Y, 2=Z.
#[derive(Clone, Copy, PartialEq, Eq, Debug)]
pub(crate) struct Dir {
    pub axis: usize,
    pub sign: i8,
}

#[derive(Clone, Copy, PartialEq, Eq, Debug)]
pub(crate) enum LayerKind {
    Face,
    Wide,
    Mid,
    All,
}

/// Orthonormal integer 3x3 matrix; `cols[a]` is the image of basis vector a.
#[derive(Clone, Copy, PartialEq, Eq, Debug)]
pub(crate) struct Rot3 {
    pub cols: [[i8; 3]; 3],
}

impl Rot3 {
    pub const IDENTITY: Rot3 = Rot3 {
        cols: [[1, 0, 0], [0, 1, 0], [0, 0, 1]],
    };

    pub fn apply(&self, v: [i8; 3]) -> [i8; 3] {
        let mut out = [0i8; 3];
        for (a, col) in self.cols.iter().enumerate() {
            for r in 0..3 {
                out[r] += col[r] * v[a];
            }
        }
        out
    }

    /// `self ∘ rhs` (apply `rhs` first, then `self`).
    pub fn then_after(&self, rhs: &Rot3) -> Rot3 {
        Rot3 {
            cols: [
                self.apply(rhs.cols[0]),
                self.apply(rhs.cols[1]),
                self.apply(rhs.cols[2]),
            ],
        }
    }

    /// Inverse (= transpose, since orthonormal).
    pub fn inverse(&self) -> Rot3 {
        let mut cols = [[0i8; 3]; 3];
        for a in 0..3 {
            for r in 0..3 {
                cols[a][r] = self.cols[r][a];
            }
        }
        Rot3 { cols }
    }
}

/// +90° right-hand rotations about +X, +Y, +Z.
const ROT_P90: [Rot3; 3] = [
    Rot3 {
        cols: [[1, 0, 0], [0, 0, 1], [0, -1, 0]],
    },
    Rot3 {
        cols: [[0, 0, -1], [0, 1, 0], [1, 0, 0]],
    },
    Rot3 {
        cols: [[0, 1, 0], [-1, 0, 0], [0, 0, 1]],
    },
];

impl Dir {
    pub fn vec(self) -> [i8; 3] {
        let mut v = [0i8; 3];
        v[self.axis] = self.sign;
        v
    }

    pub fn from_vec(v: [i8; 3]) -> Dir {
        for axis in 0..3 {
            if v[axis] != 0 {
                debug_assert!(v[(axis + 1) % 3] == 0 && v[(axis + 2) % 3] == 0);
                return Dir {
                    axis,
                    sign: v[axis],
                };
            }
        }
        unreachable!("zero vector is not a direction")
    }

    /// One clockwise quarter turn about this oriented axis (viewed from
    /// outside along `-self`): a -90° right-hand rotation about `self`.
    pub fn cw_quarter(self) -> Rot3 {
        if self.sign > 0 {
            ROT_P90[self.axis].inverse()
        } else {
            ROT_P90[self.axis]
        }
    }

    /// Rotation matrix of a whole-cube move about this axis with `turns`.
    pub fn rotation(self, turns: Turns) -> Rot3 {
        let q = self.cw_quarter();
        let mut m = Rot3::IDENTITY;
        for _ in 0..turns.quarters() {
            m = q.then_after(&m);
        }
        m
    }
}

impl crate::Face {
    pub(crate) fn dir(self) -> Dir {
        use crate::Face::*;
        match self {
            U => Dir { axis: 1, sign: 1 },
            R => Dir { axis: 0, sign: 1 },
            F => Dir { axis: 2, sign: 1 },
            D => Dir { axis: 1, sign: -1 },
            L => Dir { axis: 0, sign: -1 },
            B => Dir { axis: 2, sign: -1 },
        }
    }

    pub(crate) fn from_dir(d: Dir) -> crate::Face {
        use crate::Face::*;
        match (d.axis, d.sign) {
            (1, 1) => U,
            (0, 1) => R,
            (2, 1) => F,
            (1, -1) => D,
            (0, -1) => L,
            (2, -1) => B,
            _ => unreachable!(),
        }
    }
}

impl Move {
    pub fn inverse(self) -> Move {
        match self {
            Move::Face(f, t) => Move::Face(f, t.inverse()),
            Move::Wide(f, t) => Move::Wide(f, t.inverse()),
            Move::Slice(s, t) => Move::Slice(s, t.inverse()),
            Move::Rot(a, t) => Move::Rot(a, t.inverse()),
        }
    }

    pub(crate) fn gen(self) -> (Dir, LayerKind, Turns) {
        match self {
            Move::Face(f, t) => (f.dir(), LayerKind::Face, t),
            Move::Wide(f, t) => (f.dir(), LayerKind::Wide, t),
            Move::Slice(SliceKind::M, t) => (Dir { axis: 0, sign: -1 }, LayerKind::Mid, t),
            Move::Slice(SliceKind::E, t) => (Dir { axis: 1, sign: -1 }, LayerKind::Mid, t),
            Move::Slice(SliceKind::S, t) => (Dir { axis: 2, sign: 1 }, LayerKind::Mid, t),
            Move::Rot(RotAxis::X, t) => (Dir { axis: 0, sign: 1 }, LayerKind::All, t),
            Move::Rot(RotAxis::Y, t) => (Dir { axis: 1, sign: 1 }, LayerKind::All, t),
            Move::Rot(RotAxis::Z, t) => (Dir { axis: 2, sign: 1 }, LayerKind::All, t),
        }
    }

    pub(crate) fn from_gen(dir: Dir, layer: LayerKind, turns: Turns) -> Move {
        match layer {
            LayerKind::Face => Move::Face(crate::Face::from_dir(dir), turns),
            LayerKind::Wide => Move::Wide(crate::Face::from_dir(dir), turns),
            LayerKind::Mid => {
                // Canonical slice directions: M = -X, E = -Y, S = +Z.
                let (kind, canonical_sign) = match dir.axis {
                    0 => (SliceKind::M, -1),
                    1 => (SliceKind::E, -1),
                    _ => (SliceKind::S, 1),
                };
                let t = if dir.sign == canonical_sign {
                    turns
                } else {
                    turns.inverse()
                };
                Move::Slice(kind, t)
            }
            LayerKind::All => {
                let axis = match dir.axis {
                    0 => RotAxis::X,
                    1 => RotAxis::Y,
                    _ => RotAxis::Z,
                };
                let t = if dir.sign > 0 { turns } else { turns.inverse() };
                Move::Rot(axis, t)
            }
        }
    }

    /// The same physical move relabeled through a whole-cube rotation:
    /// conjugation `g⁻¹ ∘ m ∘ g`, where `g` has active matrix `g_active`.
    pub(crate) fn conjugated(self, g_active_inv: &Rot3) -> Move {
        let (dir, layer, turns) = self.gen();
        let new_dir = Dir::from_vec(g_active_inv.apply(dir.vec()));
        Move::from_gen(new_dir, layer, turns)
    }

    /// Stable index 0..54 over all move variants (6 faces + 6 wide +
    /// 3 slices + 3 rotations, each × 3 turn amounts).
    pub fn index(self) -> usize {
        let t = |t: Turns| (t.quarters() - 1) as usize;
        match self {
            Move::Face(f, tt) => (f as usize) * 3 + t(tt),
            Move::Wide(f, tt) => 18 + (f as usize) * 3 + t(tt),
            Move::Slice(s, tt) => 36 + (s as usize) * 3 + t(tt),
            Move::Rot(a, tt) => 45 + (a as usize) * 3 + t(tt),
        }
    }

    pub fn from_index(i: usize) -> Move {
        use crate::Face;
        let turns = [Turns::Cw, Turns::Half, Turns::Ccw][i % 3];
        let faces = [Face::U, Face::R, Face::F, Face::D, Face::L, Face::B];
        match i / 3 {
            k @ 0..=5 => Move::Face(faces[k], turns),
            k @ 6..=11 => Move::Wide(faces[k - 6], turns),
            k @ 12..=14 => Move::Slice([SliceKind::M, SliceKind::E, SliceKind::S][k - 12], turns),
            k @ 15..=17 => Move::Rot([RotAxis::X, RotAxis::Y, RotAxis::Z][k - 15], turns),
            _ => unreachable!(),
        }
    }
}

impl fmt::Display for Move {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Move::Face(face, t) => write!(f, "{face:?}{}", t.suffix()),
            Move::Wide(face, t) => write!(f, "{face:?}w{}", t.suffix()),
            Move::Slice(s, t) => write!(f, "{s:?}{}", t.suffix()),
            Move::Rot(a, t) => {
                let c = match a {
                    RotAxis::X => 'x',
                    RotAxis::Y => 'y',
                    RotAxis::Z => 'z',
                };
                write!(f, "{c}{}", t.suffix())
            }
        }
    }
}

#[derive(Clone, PartialEq, Eq, Debug)]
pub struct ParseError {
    pub position: usize,
    pub message: String,
}

impl fmt::Display for ParseError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(f, "parse error at char {}: {}", self.position, self.message)
    }
}

impl std::error::Error for ParseError {}

/// A sequence of moves.
#[derive(Clone, PartialEq, Eq, Debug, Default)]
pub struct Alg(pub Vec<Move>);

impl Alg {
    pub fn new(moves: Vec<Move>) -> Alg {
        Alg(moves)
    }

    /// Parse standard notation: `U U' U2 Uw u M E S x y z`, with groups and
    /// repetition/inversion: `(R U R' U')2`, `(R U R' U')'`.
    pub fn parse(s: &str) -> Result<Alg, ParseError> {
        let chars: Vec<char> = s.chars().collect();
        let mut i = 0usize;
        let moves = parse_seq(&chars, &mut i, 0)?;
        if i < chars.len() {
            return Err(ParseError {
                position: i,
                message: format!("unexpected '{}'", chars[i]),
            });
        }
        Ok(Alg(moves))
    }

    pub fn inverse(&self) -> Alg {
        Alg(self.0.iter().rev().map(|m| m.inverse()).collect())
    }

    pub fn is_empty(&self) -> bool {
        self.0.is_empty()
    }

    pub fn len(&self) -> usize {
        self.0.len()
    }

    /// The algorithm relabeled to execute in a cube frame rotated by `y^j`:
    /// applying the result to `s` equals `y⁻ʲ(alg(yʲ(s)))`.
    pub fn in_y_frame(&self, j: u8) -> Alg {
        let y = Move::Rot(RotAxis::Y, Turns::Cw).gen().0.rotation(Turns::Cw);
        let mut g = Rot3::IDENTITY;
        for _ in 0..(j % 4) {
            g = y.then_after(&g);
        }
        let g_inv = g.inverse();
        Alg(self.0.iter().map(|m| m.conjugated(&g_inv)).collect())
    }

    /// Equivalent sequence of outer face moves only. The result reproduces
    /// the same state **up to a whole-cube rotation** (rotations are folded
    /// out; wide and slice moves are expanded).
    pub fn face_moves_only(&self) -> Alg {
        let mut rho = Rot3::IDENTITY; // accumulated whole-cube rotation
        let mut out = Vec::with_capacity(self.0.len() * 2);
        for &m in &self.0 {
            let m = m.conjugated(&rho.inverse());
            let (dir, layer, turns) = m.gen();
            match layer {
                LayerKind::Face => out.push(m),
                LayerKind::Wide => {
                    // Wide(d,t) = All(d,t) ∘ Face(-d,t)
                    out.push(Move::from_gen(
                        Dir {
                            axis: dir.axis,
                            sign: -dir.sign,
                        },
                        LayerKind::Face,
                        turns,
                    ));
                    rho = rho.then_after(&dir.rotation(turns));
                }
                LayerKind::Mid => {
                    // Mid(d,t) = All(d,t) ∘ Face(d,t⁻¹) ∘ Face(-d,t)
                    out.push(Move::from_gen(dir, LayerKind::Face, turns.inverse()));
                    out.push(Move::from_gen(
                        Dir {
                            axis: dir.axis,
                            sign: -dir.sign,
                        },
                        LayerKind::Face,
                        turns,
                    ));
                    rho = rho.then_after(&dir.rotation(turns));
                }
                LayerKind::All => {
                    rho = rho.then_after(&dir.rotation(turns));
                }
            }
        }
        Alg(out)
    }

    /// Half-turn-metric length (wide = 1, slice = 2, rotations = 0).
    pub fn len_htm(&self) -> usize {
        self.0
            .iter()
            .map(|m| match m {
                Move::Face(..) | Move::Wide(..) => 1,
                Move::Slice(..) => 2,
                Move::Rot(..) => 0,
            })
            .sum()
    }
}

impl fmt::Display for Alg {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        for (i, m) in self.0.iter().enumerate() {
            if i > 0 {
                f.write_str(" ")?;
            }
            write!(f, "{m}")?;
        }
        Ok(())
    }
}

fn parse_seq(chars: &[char], i: &mut usize, depth: usize) -> Result<Vec<Move>, ParseError> {
    let mut out = Vec::new();
    loop {
        while *i < chars.len() && chars[*i].is_whitespace() {
            *i += 1;
        }
        if *i >= chars.len() {
            return Ok(out);
        }
        let c = chars[*i];
        if c == ')' {
            if depth == 0 {
                return Err(ParseError {
                    position: *i,
                    message: "unmatched ')'".into(),
                });
            }
            return Ok(out);
        }
        if c == '(' {
            *i += 1;
            let inner = parse_seq(chars, i, depth + 1)?;
            if *i >= chars.len() || chars[*i] != ')' {
                return Err(ParseError {
                    position: *i,
                    message: "expected ')'".into(),
                });
            }
            *i += 1;
            // Optional repetition count, optional inversion prime.
            let mut reps = 1usize;
            if *i < chars.len() && chars[*i].is_ascii_digit() {
                reps = chars[*i].to_digit(10).unwrap() as usize;
                if reps == 0 {
                    return Err(ParseError {
                        position: *i,
                        message: "repetition count must be >= 1".into(),
                    });
                }
                *i += 1;
            }
            let inverted = if *i < chars.len() && is_prime(chars[*i]) {
                *i += 1;
                true
            } else {
                false
            };
            let unit = if inverted {
                Alg(inner).inverse().0
            } else {
                inner
            };
            for _ in 0..reps {
                out.extend_from_slice(&unit);
            }
            continue;
        }
        out.push(parse_move(chars, i)?);
    }
}

fn is_prime(c: char) -> bool {
    c == '\'' || c == '’' || c == '`' || c == '′'
}

fn parse_move(chars: &[char], i: &mut usize) -> Result<Move, ParseError> {
    use crate::Face;
    let start = *i;
    let c = chars[*i];
    *i += 1;

    let face = |c: char| match c {
        'U' | 'u' => Some(Face::U),
        'R' | 'r' => Some(Face::R),
        'F' | 'f' => Some(Face::F),
        'D' | 'd' => Some(Face::D),
        'L' | 'l' => Some(Face::L),
        'B' | 'b' => Some(Face::B),
        _ => None,
    };

    let base = if let Some(f) = face(c) {
        if c.is_ascii_lowercase() {
            Move::Wide(f, Turns::Cw)
        } else if *i < chars.len() && chars[*i] == 'w' {
            *i += 1;
            Move::Wide(f, Turns::Cw)
        } else {
            Move::Face(f, Turns::Cw)
        }
    } else {
        match c {
            'M' => Move::Slice(SliceKind::M, Turns::Cw),
            'E' => Move::Slice(SliceKind::E, Turns::Cw),
            'S' => Move::Slice(SliceKind::S, Turns::Cw),
            'x' => Move::Rot(RotAxis::X, Turns::Cw),
            'y' => Move::Rot(RotAxis::Y, Turns::Cw),
            'z' => Move::Rot(RotAxis::Z, Turns::Cw),
            _ => {
                return Err(ParseError {
                    position: start,
                    message: format!("unknown move '{c}'"),
                })
            }
        }
    };

    // Suffix: '2' and/or prime, in either order ("2'" == "'2" == Half inverse
    // which is still Half; lone prime = Ccw).
    let mut turns = Turns::Cw;
    if *i < chars.len() && chars[*i] == '2' {
        *i += 1;
        turns = Turns::Half;
        if *i < chars.len() && is_prime(chars[*i]) {
            *i += 1;
        }
    } else if *i < chars.len() && is_prime(chars[*i]) {
        *i += 1;
        turns = Turns::Ccw;
        if *i < chars.len() && chars[*i] == '2' {
            *i += 1;
            turns = Turns::Half;
        }
    } else if *i < chars.len() && chars[*i] == '3' {
        *i += 1;
        turns = Turns::Ccw;
    }

    Ok(match (base, turns) {
        (m, Turns::Cw) => m,
        (Move::Face(f, _), t) => Move::Face(f, t),
        (Move::Wide(f, _), t) => Move::Wide(f, t),
        (Move::Slice(s, _), t) => Move::Slice(s, t),
        (Move::Rot(a, _), t) => Move::Rot(a, t),
    })
}
