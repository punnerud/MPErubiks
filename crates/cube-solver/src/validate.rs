//! Friendly validation for scanned cubes: distinguishes fixable scan
//! mistakes (wrong color counts) from physically impossible cubes.

use cube_core::{Face, FaceletCube};
use std::fmt;

#[derive(Clone, Debug, PartialEq, Eq)]
pub enum ValidationError {
    /// Not a parseable cube at all (internal).
    BadFaceletString,
    /// Wrong number of stickers per color (a scan mistake): counts in
    /// U R F D L B order — each must be 9.
    ColorCounts([u8; 6]),
    /// The six centers are not six distinct colors.
    CentersNotDistinct,
    /// Some corner/edge has a sticker combination that no real piece has
    /// (e.g. two identical colors on one piece, or opposite colors).
    ImpossiblePiece,
    /// All pieces exist but the cube is unsolvable (twisted corner,
    /// flipped edge or swapped pieces — usually one misread sticker).
    Unsolvable,
}

impl fmt::Display for ValidationError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            ValidationError::BadFaceletString => write!(f, "not a valid cube description"),
            ValidationError::ColorCounts(c) => write!(f, "wrong sticker counts: {c:?}"),
            ValidationError::CentersNotDistinct => write!(f, "center stickers must be 6 colors"),
            ValidationError::ImpossiblePiece => write!(f, "a piece has an impossible color combination"),
            ValidationError::Unsolvable => {
                write!(f, "cube is unsolvable (twisted corner / flipped edge?)")
            }
        }
    }
}

impl std::error::Error for ValidationError {}

pub fn validate(s: &FaceletCube) -> Result<(), ValidationError> {
    let mut counts = [0u8; 6];
    for &c in &s.0 {
        counts[c as usize] += 1;
    }
    if counts != [9; 6] {
        return Err(ValidationError::ColorCounts(counts));
    }
    let mut centers: Vec<Face> = Face::ALL.iter().map(|&f| s.center(f)).collect();
    centers.sort_unstable();
    centers.dedup();
    if centers.len() != 6 {
        return Err(ValidationError::CentersNotDistinct);
    }
    // Piece existence FIRST, ourselves: kewb's facelet->cubie conversion
    // silently leaves unmatchable pieces at their defaults (it does not
    // reject a {U,U,F} corner), so an impossible cube could slip through
    // and be "solved" as the wrong cube.
    let normalized = s.normalize_orientation();
    if let e @ ValidationError::ImpossiblePiece = piece_level_diagnosis(&normalized) {
        return Err(e);
    }
    match crate::solve::to_cubie(s) {
        Ok(_) => Ok(()),
        Err(crate::SolveError::Invalid(e)) => Err(e),
        Err(_) => Err(ValidationError::BadFaceletString),
    }
}

/// When kewb rejects the piece structure: check whether every real piece
/// can be located (piece-existence) to give a more precise message.
pub(crate) fn piece_level_diagnosis(s: &FaceletCube) -> ValidationError {
    let solved = FaceletCube::SOLVED;
    for slot in 0..8 {
        let expected = solved.corner_colors(slot);
        if s.locate_corner(expected).is_none() {
            return ValidationError::ImpossiblePiece;
        }
    }
    for slot in 0..12 {
        let expected = solved.edge_colors(slot);
        if s.locate_edge(expected).is_none() {
            return ValidationError::ImpossiblePiece;
        }
    }
    ValidationError::Unsolvable
}

#[cfg(test)]
mod tests {
    use super::*;
    use cube_core::{Alg, Move, Turns};

    #[test]
    fn solved_and_scrambled_are_valid() {
        assert_eq!(validate(&FaceletCube::SOLVED), Ok(()));
        let s = FaceletCube::SOLVED.applied_alg(&Alg::parse("R U F' L2 D B'").unwrap());
        assert_eq!(validate(&s), Ok(()));
    }

    #[test]
    fn single_sticker_swap_is_caught() {
        // Swapping two stickers of different colors breaks color counts or
        // solvability, never passes.
        let mut s = FaceletCube::SOLVED;
        s.0.swap(0, 9); // a U sticker with an R sticker
        assert!(validate(&s).is_err());
    }

    #[test]
    fn flipped_edge_is_unsolvable() {
        // Swap the two stickers of the UF edge (U8 = index 7, F2 = index
        // 19): every piece still exists, but the flip sum breaks.
        let mut flipped = FaceletCube::SOLVED;
        flipped.0.swap(7, 19);
        assert_eq!(validate(&flipped), Err(ValidationError::Unsolvable));
    }

    #[test]
    fn missing_piece_is_impossible() {
        // Swap the R sticker of the URF corner (R1 = index 9) with the L
        // sticker of the ULB corner (L1 = index 36): color counts stay 9
        // each, but the {U,R,F} piece no longer exists anywhere.
        let mut s = FaceletCube::SOLVED;
        s.0.swap(9, 36);
        assert_eq!(validate(&s), Err(ValidationError::ImpossiblePiece));
    }

    #[test]
    fn rotated_solved_cube_validates() {
        let s = FaceletCube::SOLVED.applied(Move::Rot(cube_core::RotAxis::X, Turns::Cw));
        assert_eq!(validate(&s), Ok(()));
    }
}
