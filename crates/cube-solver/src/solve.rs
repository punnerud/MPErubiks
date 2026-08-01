use crate::table::table;
use cube_core::{Alg, FaceletCube};
use kewb::{CubieCube, FaceCube, Solver};
use std::fmt;

#[derive(Clone, Debug)]
pub enum SolveError {
    TableNotLoaded,
    Table(String),
    Invalid(crate::ValidationError),
    /// Should not happen for a valid cube — kewb found nothing within the
    /// move bound even at the fallback depth.
    NoSolution,
}

impl fmt::Display for SolveError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            SolveError::TableNotLoaded => write!(f, "solver table not loaded yet"),
            SolveError::Table(e) => write!(f, "solver table failed to decode: {e}"),
            SolveError::Invalid(e) => write!(f, "invalid cube: {e}"),
            SolveError::NoSolution => write!(f, "no solution found within the move bound"),
        }
    }
}

impl std::error::Error for SolveError {}

pub(crate) fn to_cubie(s: &FaceletCube) -> Result<CubieCube, SolveError> {
    // The solver assumes centers in home orientation.
    let normalized = s.normalize_orientation();
    let face = FaceCube::try_from(normalized.to_facelet_string().as_str())
        .map_err(|_| SolveError::Invalid(crate::ValidationError::BadFaceletString))?;
    CubieCube::try_from(&face).map_err(|_| {
        SolveError::Invalid(crate::validate::piece_level_diagnosis(&normalized))
    })
}

fn kewb_moves_to_alg(moves: &[kewb::Move]) -> Alg {
    // Round-trip through notation: version-proof against kewb enum layout.
    let text = moves
        .iter()
        .map(|m| m.to_string())
        .collect::<Vec<_>>()
        .join(" ");
    Alg::parse(&text).expect("kewb notation is a subset of ours")
}

/// Solve within 23 moves: kewb finds those near-instantly, while a hard
/// 21-bound can grind for 10+ seconds on one wasm thread for unlucky
/// states (field-measured). Most solutions still land <=21, the guide is
/// pedagogically identical at 22-23, and the MPEdb solution cache makes
/// every repeat instant.
/// `timeout: None` everywhere — kewb's timeout path burns the full budget
/// and uses `std::time::Instant`, which panics on wasm.
pub fn solve(s: &FaceletCube) -> Result<Alg, SolveError> {
    solve_bounded(s, 23).or_else(|e| match e {
        SolveError::NoSolution => solve_bounded(s, 23),
        other => Err(other),
    })
}

/// Public benchmarking hook: solve with an explicit move bound.
pub fn solve_bounded_public(s: &FaceletCube, max_length: u8) -> Result<Alg, SolveError> {
    solve_bounded(s, max_length)
}

pub(crate) fn solve_bounded(s: &FaceletCube, max_length: u8) -> Result<Alg, SolveError> {
    let state = to_cubie(s)?;
    let mut solver = Solver::new(table()?, max_length, None);
    let solution = solver.solve(state).ok_or(SolveError::NoSolution)?;
    Ok(kewb_moves_to_alg(&solution.get_all_moves()))
}

/// A uniformly random solvable state (WCA-style random state).
pub fn random_state() -> FaceletCube {
    let cubie = kewb::generators::generate_random_state();
    let face = FaceCube::try_from(&cubie).expect("generated state is valid");
    FaceletCube::from_facelet_string(&face.to_string()).expect("kewb facelet string is valid")
}

/// A scramble sequence that produces `s` from a solved cube (the inverse
/// of a solution — short, WCA-style).
pub fn scramble_for(s: &FaceletCube) -> Result<Alg, SolveError> {
    Ok(solve(s)?.inverse())
}

#[cfg(test)]
mod tests {
    use super::*;

    fn ensure_table() {
        if !crate::table_ready() {
            let bytes = std::fs::read(concat!(
                env!("CARGO_MANIFEST_DIR"),
                "/../../assets/table.bin"
            ))
            .expect("assets/table.bin missing — run: cargo run -p xtask -- gen-table");
            crate::install_table(&bytes).unwrap();
        }
    }

    #[test]
    fn solves_100_random_states_within_23() {
        ensure_table();
        let mut over_21 = 0;
        for i in 0..100 {
            let state = random_state();
            let alg = solve(&state).unwrap_or_else(|e| panic!("state {i}: {e}"));
            let solved = state.applied_alg(&alg);
            assert!(solved.is_solved(), "state {i}: solution does not solve");
            if alg.len_htm() > 21 {
                over_21 += 1;
            }
            assert!(alg.len_htm() <= 23, "state {i}: {} moves", alg.len_htm());
        }
        eprintln!("{over_21}/100 solutions used 22-23 moves");
    }

    #[test]
    #[ignore = "long: 1000 states; run with --ignored"]
    fn solves_1000_random_states() {
        ensure_table();
        for i in 0..1000 {
            let state = random_state();
            let alg = solve(&state).unwrap_or_else(|e| panic!("state {i}: {e}"));
            assert!(state.applied_alg(&alg).is_solved(), "state {i}");
            assert!(alg.len_htm() <= 23, "state {i}: {} moves", alg.len_htm());
        }
    }

    #[test]
    fn scramble_roundtrip() {
        ensure_table();
        let state = random_state();
        let scramble = scramble_for(&state).unwrap();
        assert_eq!(cube_core::FaceletCube::SOLVED.applied_alg(&scramble), state.normalize_orientation());
    }
}
