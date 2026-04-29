//! Solver adaptors.

pub mod minion;

pub mod rustsat;

#[path = "ortools-cpsat/mod.rs"]
pub mod ortools_cpsat;

pub mod smt;

#[doc(inline)]
pub use minion::{Minion, MinionValueOrder};

#[doc(inline)]
pub use ortools_cpsat::OrToolsCpSat;

#[doc(inline)]
pub use rustsat::Sat;

#[doc(inline)]
pub use smt::Smt;
