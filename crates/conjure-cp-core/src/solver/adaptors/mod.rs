//! Solver adaptors.

pub mod minion;
pub mod rustsat;

#[doc(inline)]
pub use minion::Minion;

#[doc(inline)]
pub use rustsat::Sat;
