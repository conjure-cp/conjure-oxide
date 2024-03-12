//! Solver adaptors.

mod sat_common;

mod minion;
#[doc(inline)]
pub use minion::Minion;

mod kissat;
#[doc(inline)]
pub use kissat::Kissat;
