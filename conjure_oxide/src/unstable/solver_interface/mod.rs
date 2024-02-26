//! A new interface for interacting with solvers.
//!
//! # Example
//!
//! TODO

// # Implementing Solver interfaces
//
// Solver interfaces can only be implemented inside this module, due to the SolverAdaptor crate
// being sealed.
//
// To add support for a solver, implement the `SolverAdaptor` trait in a submodule.
//
// If incremental solving support is required, also implement `ModelModifier`. If this is not
// required, all `ModelModifier` instances required by the SolverAdaptor trait can be replaced with
// NotModifiable.
//
// For more details, see the docstrings for SolverAdaptor, ModelModifier, and NotModifiable.

#![allow(dead_code)]
#![allow(unused)]
#![warn(clippy::exhaustive_enums)]

#[doc(hidden)]
mod private {
    // Used to limit calling trait functions outside this module.
    #[doc(hidden)]
    pub struct Internal;

    // https://predr.ag/blog/definitive-guide-to-sealed-traits-in-rust/#the-trick-for-sealing-traits
    // Make traits unimplementable from outside of this module.
    #[doc(hidden)]
    pub trait Sealed {}
}

use self::incremental::*;
use self::solver_states::*;
use anyhow::anyhow;
use conjure_core::ast::{Domain, Expression, Model, Name};
use std::collections::HashMap;
use std::error::Error;
use std::fmt::{Debug, Display};

/// A [`SolverAdaptor`] provide an interface to an underlying solver. Used by [`Solver`].
pub trait SolverAdaptor: private::Sealed {
    /// The native model type of the underlying solver.
    type Model: Clone;

    /// The native solution type of the underlying solver.
    type Solution: Clone;

    /// The [`ModelModifier`](incremental::ModelModifier) used during incremental search.
    ///
    /// If incremental solving is not supported, this SHOULD be set to [NotModifiable](`incremental::NotModifiable`) .
    type Modifier: incremental::ModelModifier;

    /// Run the solver on the given model.
    ///
    /// Implementations of this function MUST call the user provided callback whenever a solution
    /// is found. If the user callback returns `true`, search should continue, if the user callback
    /// returns `false`, search should terminate.
    fn solve(
        &mut self,
        model: Self::Model,
        callback: fn(HashMap<String, String>) -> bool,
        _: private::Internal,
    ) -> Result<ExecutionSuccess, ExecutionFailure>;

    /// Run the solver on the given model, allowing modification of the model through a
    /// [`ModelModifier`].
    ///
    /// Implementations of this function MUST return [`OpNotSupported`](`ModificationFailure::OpNotSupported`)
    /// if modifying the model mid-search is not supported. These implementations may also find the
    /// [`NotModifiable`] modifier useful.
    ///
    /// As with [`solve`](SolverAdaptor::solve), this function MUST call the user provided callback
    /// function whenever a solution is found.
    fn solve_mut(
        &mut self,
        model: Self::Model,
        callback: fn(HashMap<String, String>, Self::Modifier) -> bool,
        _: private::Internal,
    ) -> Result<ExecutionSuccess, ExecutionFailure>;
    fn load_model(
        &mut self,
        model: Model,
        _: private::Internal,
    ) -> Result<Self::Model, anyhow::Error>;
    fn init_solver(&mut self, _: private::Internal) {}
}

/// A Solver executes of a Conjure-Oxide model usign a specified solver.
pub struct Solver<A: SolverAdaptor, State: SolverState = Init> {
    state: std::marker::PhantomData<State>,
    adaptor: A,
    model: Option<A::Model>,
}

impl<A: SolverAdaptor> Solver<A, Init> {
    // TODO: decent error handling
    pub fn load_model(mut self, model: Model) -> Result<Solver<A, ModelLoaded>, ()> {
        let solver_model = &mut self
            .adaptor
            .load_model(model, private::Internal)
            .map_err(|_| ())?;
        Ok(Solver {
            state: std::marker::PhantomData::<ModelLoaded>,
            adaptor: self.adaptor,
            model: Some(solver_model.clone()),
        })
    }
}

impl<A: SolverAdaptor> Solver<A, ModelLoaded> {
    pub fn solve(
        mut self,
        callback: fn(HashMap<String, String>) -> bool,
    ) -> Result<ExecutionSuccess, ExecutionFailure> {
        #[allow(clippy::unwrap_used)]
        self.adaptor
            .solve(self.model.unwrap(), callback, private::Internal)
    }

    pub fn solve_mut(
        mut self,
        callback: fn(HashMap<String, String>, A::Modifier) -> bool,
    ) -> Result<ExecutionSuccess, ExecutionFailure> {
        #[allow(clippy::unwrap_used)]
        self.adaptor
            .solve_mut(self.model.unwrap(), callback, private::Internal)
    }
}

impl<T: SolverAdaptor> Solver<T> {
    pub fn new(solver_adaptor: T) -> Solver<T> {
        let mut solver = Solver {
            state: std::marker::PhantomData::<Init>,
            adaptor: solver_adaptor,
            model: None,
        };

        solver.adaptor.init_solver(private::Internal);
        solver
    }
}

pub mod solver_states {
    //! States of a [`Solver`].

    use super::private::Sealed;
    use super::Solver;

    pub trait SolverState: Sealed {}

    impl Sealed for Init {}
    impl Sealed for ModelLoaded {}
    impl Sealed for ExecutionSuccess {}
    impl Sealed for ExecutionFailure {}

    impl SolverState for Init {}
    impl SolverState for ModelLoaded {}
    impl SolverState for ExecutionSuccess {}
    impl SolverState for ExecutionFailure {}

    pub struct Init;
    pub struct ModelLoaded;

    /// The state returned by [`Solver`] if solving has been successful.
    pub struct ExecutionSuccess;

    /// The state returned by [`Solver`] if solving has not been successful.
    #[non_exhaustive]
    pub enum ExecutionFailure {
        /// The desired function or solver is not implemented yet.
        OpNotImplemented,

        /// The solver does not support this operation.
        OpNotSupported,

        /// Solving timed-out.
        TimedOut,

        /// An unspecified error has occurred.
        Error(anyhow::Error),
    }
}

pub mod incremental {
    //! Incremental / mutable solving (changing the model during search).
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
}
