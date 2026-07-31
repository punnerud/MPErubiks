//! Dependency-free Rubik's cube model.
//!
//! The single source of truth is a permutation of 54 facelets in Kociemba
//! order (U1..U9, R1..R9, F1..F9, D1..D9, L1..L9, B1..B9). Every move —
//! face turns, wide turns, slices and whole-cube rotations — is derived
//! geometrically from integer sticker positions, so the only hand-written
//! data is six face frames that the test suite locks down.

mod cases;
mod cubie;
mod facelet;
mod moves;
mod rng;

pub use cases::{CaseDef, CaseSet, LibraryError, Match, RecogKind, Recognizer};
pub use facelet::{face_of, sticker_index, Color, Face, FaceletCube, ParseStateError};
pub use moves::{Alg, Move, ParseError, RotAxis, SliceKind, Turns};
pub use rng::SplitMix64;
