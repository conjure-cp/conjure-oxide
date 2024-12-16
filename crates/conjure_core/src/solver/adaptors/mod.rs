//! Solver adaptors.

// #[doc(inline)]
// pub use kissat::Kissat;
#[doc(inline)]
pub use minion::Minion;
pub use rustsat::SAT;
// mod sat_common;

// mod kissat;
mod minion;
pub mod rustsat;                                //temp visibility
// mod common;
