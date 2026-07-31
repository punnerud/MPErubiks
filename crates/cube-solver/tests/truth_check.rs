//! Builds the ground-truth cube for the labeled night run from the
//! hand-read montage labels + the shipped grid tables + the physical
//! class->face identity of that cube, and validates it. A failure here
//! means the TABLES (not the classifier) disagree with reality.
use cube_core::{Face, FaceletCube};

const LABELS: [[u8; 9]; 6] = [
    [3, 1, 2, 0, 2, 1, 5, 0, 0],
    [4, 1, 1, 2, 4, 5, 2, 2, 0],
    [4, 3, 4, 1, 3, 3, 5, 2, 2],
    [2, 4, 1, 5, 0, 3, 4, 4, 3],
    [5, 4, 0, 5, 5, 0, 1, 2, 5],
    [3, 0, 3, 5, 1, 3, 0, 4, 1],
];
const ORDER: [Face; 6] = [Face::F, Face::R, Face::B, Face::D, Face::L, Face::U];
const TABLES: [[u8; 9]; 6] = [
    [0, 1, 2, 3, 4, 5, 6, 7, 8],
    [0, 1, 2, 3, 4, 5, 6, 7, 8],
    [0, 1, 2, 3, 4, 5, 6, 7, 8],
    [8, 7, 6, 5, 4, 3, 2, 1, 0],
    [6, 3, 0, 7, 4, 1, 8, 5, 2],
    [0, 1, 2, 3, 4, 5, 6, 7, 8],
];

#[test]
fn labeled_night_run_is_a_legal_cube() {
    // Physical mapping for this cube, from its centers: F=red R=green
    // B=orange D=white L=blue U=yellow (classes 0..6 = w y r o g b).
    let class_to_face = [Face::D, Face::U, Face::F, Face::B, Face::R, Face::L];
    let mut cube = FaceletCube::SOLVED;
    for (k, &face) in ORDER.iter().enumerate() {
        for (g, &off) in TABLES[k].iter().enumerate() {
            cube.0[face as usize * 9 + off as usize] =
                class_to_face[LABELS[k][g] as usize];
        }
    }
    if let Err(e) = cube_solver::validate(&cube) {
        panic!("labels+tables do not form a legal cube: {e}\n{cube:?}");
    }
}
