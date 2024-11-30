//! Solver adaptors.

// #[doc(inline)]
// pub use kissat::Kissat;
#[doc(inline)]
pub use minion::Minion;
pub use sat_adaptor::SAT;
// mod sat_common;

// mod kissat;
mod minion;
pub mod sat_adaptor;                                //temp visibility
// mod common;
