//! Solver adaptors.

mod minion;
pub mod rustsat;
use std::arch::x86_64;

#[doc(inline)]
#[doc(inline)]
pub use minion::Minion;
pub use rustsat::SAT;
