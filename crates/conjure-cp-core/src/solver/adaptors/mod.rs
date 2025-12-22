//! Solver adaptors.

pub mod minion;
pub mod rustsat;

#[doc(inline)]
pub use minion::Minion;

#[doc(inline)]
pub use rustsat::Sat;

#[cfg(feature = "smt")]
pub mod smt;

#[cfg(feature = "smt")]
#[doc(inline)]
pub use smt::Smt;
