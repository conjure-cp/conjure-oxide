mod error;

pub mod minion;
pub mod kissat;

pub use crate::ast::Model;
pub use error::*;

pub trait FromConjureModel
where
    Self: Sized,
{
    fn from_conjure(model: Model) -> Result<Self, SolverError>;
}
