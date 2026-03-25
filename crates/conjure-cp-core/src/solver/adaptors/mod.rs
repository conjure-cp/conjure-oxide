//! Solver adaptors.

pub mod minion;
pub mod rustsat;
pub mod savile_row;

#[doc(inline)]
pub use minion::Minion;

#[doc(inline)]
pub use rustsat::Sat;

#[doc(inline)]
pub use savile_row::SavileRow;
pub mod smt;

#[doc(inline)]
pub use smt::Smt;
