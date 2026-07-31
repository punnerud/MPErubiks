//! Facelet-level cube state and geometrically derived move permutations.
//!
//! Sticker positions are integer points on the cube surface (tangent
//! coordinates in {-2, 0, 2}, normal coordinate ±3, axes X→R, Y→U, Z→F).
//! A move's permutation is derived by rotating the affected stickers'
//! positions with exact integer matrices — the only hand-written data is
//! the six face frames below, which the test suite locks down.

use crate::moves::{Alg, Dir, LayerKind, Move, RotAxis, Turns};
use std::fmt;
use std::sync::OnceLock;

/// A face of the cube, in Kociemba facelet-string order.
#[repr(u8)]
#[derive(Clone, Copy, PartialEq, Eq, Hash, PartialOrd, Ord, Debug)]
pub enum Face {
    U = 0,
    R = 1,
    F = 2,
    D = 3,
    L = 4,
    B = 5,
}

/// A sticker color, named by the face it belongs to on a solved cube.
pub type Color = Face;

impl Face {
    pub const ALL: [Face; 6] = [Face::U, Face::R, Face::F, Face::D, Face::L, Face::B];

    pub fn from_index(i: usize) -> Face {
        Face::ALL[i]
    }

    pub fn letter(self) -> char {
        b"URFDLB"[self as usize] as char
    }

    pub fn from_letter(c: char) -> Option<Face> {
        match c {
            'U' => Some(Face::U),
            'R' => Some(Face::R),
            'F' => Some(Face::F),
            'D' => Some(Face::D),
            'L' => Some(Face::L),
            'B' => Some(Face::B),
            _ => None,
        }
    }
}

/// Which face a facelet index belongs to.
pub fn face_of(index: usize) -> Face {
    Face::from_index(index / 9)
}

/// The facelet index shown on `face` of the cubie at grid coordinate `q`
/// (each component -1, 0 or 1), if that cubie has a sticker there.
/// Used by renderers to map cube state onto cubie geometry.
pub fn sticker_index(q: [i8; 3], face: Face) -> Option<usize> {
    let g = geom();
    let grid = ((q[0] + 1) * 9 + (q[1] + 1) * 3 + (q[2] + 1)) as u8;
    (0..54).find(|&i| g.cubie_grid[i] == grid && face_of(i) == face)
}

/// Face frames: (normal, right, down) unit vectors. Row-major facelet
/// (row, col) sits at `3*normal + 2*(col-1)*right + 2*(row-1)*down`.
/// These encode the Kociemba layout (U1 back-left, F1 top-left, D1
/// front-left, B1 top-right seen from the front, ...).
const FRAMES: [([i8; 3], [i8; 3], [i8; 3]); 6] = [
    ([0, 1, 0], [1, 0, 0], [0, 0, 1]),    // U
    ([1, 0, 0], [0, 0, -1], [0, -1, 0]),  // R
    ([0, 0, 1], [1, 0, 0], [0, -1, 0]),   // F
    ([0, -1, 0], [1, 0, 0], [0, 0, -1]),  // D
    ([-1, 0, 0], [0, 0, 1], [0, -1, 0]),  // L
    ([0, 0, -1], [-1, 0, 0], [0, -1, 0]), // B
];

pub(crate) struct Geom {
    /// Sticker center position per facelet index.
    pub pos: [[i8; 3]; 54],
    /// Permutation per `Move::index()`: `new[i] = old[perm[i]]`.
    pub perms: Vec<[u8; 54]>,
    /// Cubie grid index per facelet: `(qx+1)*9 + (qy+1)*3 + (qz+1)`.
    pub cubie_grid: [u8; 54],
    /// Corner slots: facelet triples, U/D facelet first, then by index.
    pub corner_slots: [[u8; 3]; 8],
    /// Edge slots: facelet pairs sorted by index.
    pub edge_slots: [[u8; 2]; 12],
}

static GEOM: OnceLock<Geom> = OnceLock::new();

pub(crate) fn geom() -> &'static Geom {
    GEOM.get_or_init(Geom::build)
}

impl Geom {
    fn build() -> Geom {
        let mut pos = [[0i8; 3]; 54];
        for f in 0..6 {
            let (n, right, down) = FRAMES[f];
            for row in 0..3i8 {
                for col in 0..3i8 {
                    let idx = f * 9 + (row * 3 + col) as usize;
                    for a in 0..3 {
                        pos[idx][a] = 3 * n[a] + 2 * (col - 1) * right[a] + 2 * (row - 1) * down[a];
                    }
                }
            }
        }

        let index_of = |p: [i8; 3]| -> u8 {
            pos.iter()
                .position(|q| *q == p)
                .expect("rotated sticker position must exist") as u8
        };

        let mut perms = Vec::with_capacity(54);
        for mi in 0..54 {
            let (dir, layer, turns) = Move::from_index(mi).gen();
            let quarter = derive_quarter(&pos, &index_of, dir, layer);
            let mut perm: [u8; 54] = core::array::from_fn(|i| i as u8);
            for _ in 0..turns.quarters() {
                let mut next = [0u8; 54];
                for i in 0..54 {
                    next[i] = perm[quarter[i] as usize];
                }
                perm = next;
            }
            perms.push(perm);
        }

        let mut cubie_grid = [0u8; 54];
        for i in 0..54 {
            let q: [i8; 3] = core::array::from_fn(|a| pos[i][a].signum());
            cubie_grid[i] = ((q[0] + 1) * 9 + (q[1] + 1) * 3 + (q[2] + 1)) as u8;
        }

        let mut groups: Vec<Vec<u8>> = vec![Vec::new(); 27];
        for i in 0..54u8 {
            groups[cubie_grid[i as usize] as usize].push(i);
        }
        let mut corner_slots = Vec::new();
        let mut edge_slots = Vec::new();
        for (grid, g) in groups.iter().enumerate() {
            match g.len() {
                3 => {
                    let mut triple: Vec<u8> = g.clone();
                    triple.sort_unstable();
                    // U/D facelet first.
                    let ud = triple
                        .iter()
                        .position(|&i| matches!(face_of(i as usize), Face::U | Face::D))
                        .expect("corner has a U/D sticker");
                    let first = triple.remove(ud);
                    corner_slots.push((grid, [first, triple[0], triple[1]]));
                }
                2 => {
                    let mut pair: Vec<u8> = g.clone();
                    pair.sort_unstable();
                    edge_slots.push((grid, [pair[0], pair[1]]));
                }
                _ => {} // centers (1 facelet) and the hidden core (0)
            }
        }
        corner_slots.sort_by_key(|(grid, _)| *grid);
        edge_slots.sort_by_key(|(grid, _)| *grid);

        Geom {
            pos,
            perms,
            cubie_grid,
            corner_slots: core::array::from_fn(|i| corner_slots[i].1),
            edge_slots: core::array::from_fn(|i| edge_slots[i].1),
        }
    }
}

fn derive_quarter(
    pos: &[[i8; 3]; 54],
    index_of: &dyn Fn([i8; 3]) -> u8,
    dir: Dir,
    layer: LayerKind,
) -> [u8; 54] {
    let rot = dir.cw_quarter();
    let mut perm: [u8; 54] = core::array::from_fn(|i| i as u8);
    for i in 0..54 {
        let p = pos[i];
        let c = p[dir.axis] * dir.sign;
        let in_layer = match layer {
            LayerKind::Face => c >= 2,
            LayerKind::Wide => c >= 0,
            LayerKind::Mid => c == 0,
            LayerKind::All => true,
        };
        if in_layer {
            let j = index_of(rot.apply(p));
            perm[j as usize] = i as u8;
        }
    }
    perm
}

#[derive(Clone, PartialEq, Eq, Debug)]
pub struct ParseStateError(pub String);

impl fmt::Display for ParseStateError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(f, "invalid facelet string: {}", self.0)
    }
}

impl std::error::Error for ParseStateError {}

/// Cube state: 54 sticker colors in Kociemba facelet order.
#[derive(Clone, Copy, PartialEq, Eq, Hash, Debug)]
pub struct FaceletCube(pub [Color; 54]);

impl FaceletCube {
    pub const SOLVED: FaceletCube = {
        let mut arr = [Face::U; 54];
        let mut i = 0;
        while i < 54 {
            arr[i] = match i / 9 {
                0 => Face::U,
                1 => Face::R,
                2 => Face::F,
                3 => Face::D,
                4 => Face::L,
                _ => Face::B,
            };
            i += 1;
        }
        FaceletCube(arr)
    };

    pub fn apply(&mut self, m: Move) {
        let perm = &geom().perms[m.index()];
        let old = self.0;
        for i in 0..54 {
            self.0[i] = old[perm[i] as usize];
        }
    }

    #[must_use]
    pub fn applied(mut self, m: Move) -> FaceletCube {
        self.apply(m);
        self
    }

    pub fn apply_alg(&mut self, alg: &Alg) {
        for &m in &alg.0 {
            self.apply(m);
        }
    }

    #[must_use]
    pub fn applied_alg(mut self, alg: &Alg) -> FaceletCube {
        self.apply_alg(alg);
        self
    }

    /// Whole-cube rotation by `y` (like a U turn of the entire cube),
    /// `quarter_turns` times.
    #[must_use]
    pub fn rotate_y(&self, quarter_turns: u8) -> FaceletCube {
        let mut s = *self;
        match quarter_turns % 4 {
            0 => {}
            1 => s.apply(Move::Rot(RotAxis::Y, Turns::Cw)),
            2 => s.apply(Move::Rot(RotAxis::Y, Turns::Half)),
            _ => s.apply(Move::Rot(RotAxis::Y, Turns::Ccw)),
        }
        s
    }

    pub fn center(&self, f: Face) -> Color {
        self.0[f as usize * 9 + 4]
    }

    /// Apply `quarter_turns` clockwise U turns.
    #[must_use]
    pub fn rotate_u(&self, quarter_turns: u8) -> FaceletCube {
        let mut s = *self;
        match quarter_turns % 4 {
            0 => {}
            1 => s.apply(Move::Face(Face::U, Turns::Cw)),
            2 => s.apply(Move::Face(Face::U, Turns::Half)),
            _ => s.apply(Move::Face(Face::U, Turns::Ccw)),
        }
        s
    }

    /// 54-char Kociemba facelet string ("UUUUUUUUURRR...").
    pub fn to_facelet_string(&self) -> String {
        self.0.iter().map(|c| c.letter()).collect()
    }

    pub fn from_facelet_string(s: &str) -> Result<FaceletCube, ParseStateError> {
        let chars: Vec<char> = s.trim().chars().collect();
        if chars.len() != 54 {
            return Err(ParseStateError(format!(
                "expected 54 characters, got {}",
                chars.len()
            )));
        }
        let mut arr = [Face::U; 54];
        for (i, c) in chars.iter().enumerate() {
            arr[i] = Face::from_letter(*c)
                .ok_or_else(|| ParseStateError(format!("bad character '{c}' at {i}")))?;
        }
        Ok(FaceletCube(arr))
    }

    /// All stickers match their face's center.
    pub fn is_solved(&self) -> bool {
        (0..54).all(|i| self.0[i] == self.center(face_of(i)))
    }

    /// Bottom two layers (all cubies with y <= 0) match their centers.
    pub fn is_f2l_solved(&self) -> bool {
        let g = geom();
        (0..54).all(|i| {
            let qy = (g.cubie_grid[i] / 3) % 3; // 0 = bottom, 1 = middle, 2 = top
            qy == 2 || self.0[i] == self.center(face_of(i))
        })
    }

    /// The U face is uniform (orientation of the last layer done).
    pub fn is_oll_done(&self) -> bool {
        let c = self.center(Face::U);
        self.0[0..9].iter().all(|&x| x == c)
    }

    /// Everything except the last layer and the FR corner/edge pair matches
    /// its center (the precondition for an F2L "insert FR pair" case).
    pub fn is_f2l_minus_fr_solved(&self) -> bool {
        const FR_CORNER_GRID: u8 = 2 * 9 + 2; // q = (1, -1, 1)
        const FR_EDGE_GRID: u8 = 2 * 9 + 3 + 2; // q = (1, 0, 1)
        let g = geom();
        (0..54).all(|i| {
            let grid = g.cubie_grid[i];
            let qy = (grid / 3) % 3;
            qy == 2
                || grid == FR_CORNER_GRID
                || grid == FR_EDGE_GRID
                || self.0[i] == self.center(face_of(i))
        })
    }

    /// Rotate the whole cube so every center is on its home face.
    /// For any legal cube the centers form one of 24 rotations of solved;
    /// ILLEGAL center arrangements (a bad camera scan) return the state
    /// unchanged — validation reports them properly, panicking must not.
    #[must_use]
    pub fn normalize_orientation(&self) -> FaceletCube {
        let centers_home =
            |s: &FaceletCube| Face::ALL.iter().all(|&f| s.center(f) == f);
        if centers_home(self) {
            return *self;
        }
        use Move::Rot;
        use RotAxis::*;
        use Turns::*;
        let tops: [&[Move]; 6] = [
            &[],
            &[Rot(X, Cw)],
            &[Rot(X, Half)],
            &[Rot(X, Ccw)],
            &[Rot(Z, Cw)],
            &[Rot(Z, Ccw)],
        ];
        let spins: [&[Move]; 4] = [&[], &[Rot(Y, Cw)], &[Rot(Y, Half)], &[Rot(Y, Ccw)]];
        for top in tops {
            for spin in spins {
                let mut s = *self;
                for &m in top.iter().chain(spin.iter()) {
                    s.apply(m);
                }
                if centers_home(&s) {
                    return s;
                }
            }
        }
        // Scanned garbage (duplicate centers): leave as-is for validation.
        *self
    }
}

impl fmt::Display for FaceletCube {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.write_str(&self.to_facelet_string())
    }
}
