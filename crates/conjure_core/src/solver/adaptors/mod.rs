//! Solver adaptors.

#[doc(inline)]
pub use kissat::Kissat;
#[doc(inline)]
pub use minion::Minion;

mod sat_common;

mod kissat;
mod minion;
