//! Modifying a model during search.
//!
//! Incremental solving can be triggered for a solverthrough the
//! [`Solver::solve_mut`] method.
//!
//! This gives access to a [`ModelModifier`] in the solution retrieval callback.

use super::private;
use super::Solver;
use conjure_core::ast::{Domain, Expression, Model, Name};

/// A ModelModifier provides an interface to modify a model during solving.
///
/// Modifications are defined in terms of Conjure AST nodes, so must be translated to a solver
/// specfic form before use.
///
/// It is implementation defined whether these constraints can be given at high level and passed
/// through the rewriter, or only low-level solver constraints are supported.
///
/// See also: [`Solver::solve_mut`].
pub trait ModelModifier: private::Sealed {
    fn add_constraint(constraint: Expression) -> Result<(), ModificationFailure> {
        Err(ModificationFailure::OpNotSupported)
    }

    fn add_variable(name: Name, domain: Domain) -> Result<(), ModificationFailure> {
        Err(ModificationFailure::OpNotSupported)
    }
}

/// A [`ModelModifier`] for a solver that does not support incremental solving. Returns
/// [`OperationNotSupported`](`ModificationFailure::OperationNotSupported`) for all operations.
pub struct NotModifiable;

impl private::Sealed for NotModifiable {}
impl ModelModifier for NotModifiable {}

/// The requested modification to the model has failed.
#[non_exhaustive]
pub enum ModificationFailure {
    /// The desired operation is not supported for this solver adaptor.
    OpNotSupported,

    /// The desired operation is supported by this solver adaptor, but has not been
    /// implemented yet.
    OpNotImplemented,

    // The arguments given to the operation are invalid.
    ArgsInvalid(anyhow::Error),

    /// An unspecified error has occurred.
    Error(anyhow::Error),
}
