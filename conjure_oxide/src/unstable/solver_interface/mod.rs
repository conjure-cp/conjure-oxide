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

pub mod model_modifier;
pub mod stats;

#[doc(hidden)]
mod private;
mod solver_states;

use self::model_modifier::*;
use self::solver_states::*;
use self::stats::Stats;
use anyhow::anyhow;
use conjure_core::ast::{Domain, Expression, Model, Name};
use itertools::Either;
use std::collections::HashMap;
use std::error::Error;
use std::fmt::{Debug, Display};

/// A [`SolverAdaptor`] provide an interface to an underlying solver, used by [Solver].
pub trait SolverAdaptor: private::Sealed {
    /// The native model type of the underlying solver.
    type Model: Clone;

    /// The native solution type of the underlying solver.
    type Solution: Clone;

    /// The [`ModelModifier`](model_modifier::ModelModifier) used during incremental search.
    ///
    /// If incremental solving is not supported, this SHOULD be set to [NotModifiable](model_modifier::NotModifiable) .
    type Modifier: model_modifier::ModelModifier;

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

/// A Solver executes a Conjure-Oxide model using a given [SolverAdaptor].
#[derive(Clone)]
pub struct Solver<A: SolverAdaptor, State: SolverState = Init> {
    state: State,
    adaptor: A,
    model: Option<A::Model>,
}

impl<Adaptor: SolverAdaptor> Solver<Adaptor> {
    pub fn new(solver_adaptor: Adaptor) -> Solver<Adaptor> {
        let mut solver = Solver {
            state: Init,
            adaptor: solver_adaptor,
            model: None,
        };

        solver.adaptor.init_solver(private::Internal);
        solver
    }
}

impl<A: SolverAdaptor> Solver<A, Init> {
    // TODO: decent error handling
    pub fn load_model(mut self, model: Model) -> Result<Solver<A, ModelLoaded>, ()> {
        let solver_model = &mut self
            .adaptor
            .load_model(model, private::Internal)
            .map_err(|_| ())?;
        Ok(Solver {
            state: ModelLoaded,
            adaptor: self.adaptor,
            model: Some(solver_model.clone()),
        })
    }
}

impl<A: SolverAdaptor> Solver<A, ModelLoaded> {
    pub fn solve(
        mut self,
        callback: fn(HashMap<String, String>) -> bool,
    ) -> Either<Solver<A, ExecutionSuccess>, Solver<A, ExecutionFailure>> {
        #[allow(clippy::unwrap_used)]
        let result = self
            .adaptor
            .solve(self.model.clone().unwrap(), callback, private::Internal);

        match result {
            Ok(x) => Either::Left(Solver {
                adaptor: self.adaptor,
                model: self.model,
                state: x,
            }),
            Err(x) => Either::Right(Solver {
                adaptor: self.adaptor,
                model: self.model,
                state: x,
            }),
        }
    }

    pub fn solve_mut(
        mut self,
        callback: fn(HashMap<String, String>, A::Modifier) -> bool,
    ) -> Either<Solver<A, ExecutionSuccess>, Solver<A, ExecutionFailure>> {
        #[allow(clippy::unwrap_used)]
        let result =
            self.adaptor
                .solve_mut(self.model.clone().unwrap(), callback, private::Internal);

        match result {
            Ok(x) => Either::Left(Solver {
                adaptor: self.adaptor,
                model: self.model,
                state: x,
            }),
            Err(x) => Either::Right(Solver {
                adaptor: self.adaptor,
                model: self.model,
                state: x,
            }),
        }
    }
}

impl<A: SolverAdaptor> Solver<A, ExecutionSuccess> {
    pub fn stats(self) -> Box<dyn Stats> {
        self.state.stats
    }
}

impl<A: SolverAdaptor> Solver<A, ExecutionFailure> {
    pub fn why(self) -> ExecutionFailure {
        self.state
    }
}
