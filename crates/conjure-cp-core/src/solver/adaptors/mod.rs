//! Solver adaptors.

pub mod minion;
pub mod rustsat;
pub mod savilerow;

#[doc(inline)]
pub use minion::{Minion, MinionValueOrder};

#[doc(inline)]
pub use rustsat::Sat;

#[doc(inline)]
pub use savilerow::SavileRow;
pub mod smt;

#[doc(inline)]
pub use smt::Smt;
