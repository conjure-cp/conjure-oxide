//! Solver adaptors.

mod minion;
mod rustsat;

#[doc(inline)]
pub use minion::Minion;

#[doc(inline)]
pub use rustsat::SAT;
