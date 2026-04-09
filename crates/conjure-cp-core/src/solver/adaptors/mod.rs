//! Solver adaptors.

pub mod minion;
pub mod rustsat;

#[doc(inline)]
pub use minion::Minion;

#[doc(inline)]
pub use rustsat::Sat;

pub mod smt;

#[doc(inline)]
pub use smt::Smt;
