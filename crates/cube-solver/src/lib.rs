//! Solver integration (kewb two-phase) + hint engine.
//!
//! The move/pruning table is installed once (`install_table`) from bytes:
//! natively via `include_bytes!`, in the browser from a `fetch()`ed asset.
//! Never generate tables at runtime — multi-second cost on the UI thread.

mod hints;
mod cross;
mod my_way;
mod practice_scramble;
mod resolve_scan;
mod table_pack;
mod solve;
mod table;
mod validate;

pub use hints::{solve_with_hints, GuidedSolution, HintAt, Segment, SolveOutput};
pub use cross::solve_cross;
pub use my_way::{my_way, MySeg, MyWayPlan};
pub use practice_scramble::{practice_scramble, Mode as ScrambleMode, PracticeScramble};
pub use resolve_scan::{assign_classes, relabel_to_standard, resolve_scan, Shares};
pub use table_pack::{decode_packed, encode_packed};
pub use solve::{random_state, scramble_for, solve, solve_bounded_public, SolveError};
pub use table::{install_table, table_ready, install_packed_table};
pub use validate::{validate, ValidationError};
