//! Solver adaptors.

mod minion;
mod rustsat;
use std::arch::x86_64;

#[doc(inline)]
pub use minion::Minion;

#[doc(inline)]
pub use rustsat::SAT;
