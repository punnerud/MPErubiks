//! Solver integration (kewb two-phase) + hint engine.
//!
//! The move/pruning table is installed once (`install_table`) from bytes:
//! natively via `include_bytes!`, in the browser from a `fetch()`ed asset.
//! Never generate tables at runtime — multi-second cost on the UI thread.

mod hints;
mod solve;
mod table;
mod validate;

pub use hints::{solve_with_hints, GuidedSolution, HintAt, Segment, SolveOutput};
pub use solve::{random_state, scramble_for, solve, SolveError};
pub use table::{install_table, table_ready};
pub use validate::{validate, ValidationError};
