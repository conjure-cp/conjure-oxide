//! A high-level API for interacting with constraints solvers.
//!
//! This module provides a consistent, solver-independent API for interacting with constraints
//! solvers. It also provides incremental solving support, and the returning of run stats from
//! solvers.
//!
//! -----
//!
//! - [Solver<Adaptor>] provides the API for interacting with constraints solvers.
//!
//! - The [SolverAdaptor] trait controls how solving actually occurs and handles translation
//! between the [Solver] type and a specific solver.
//!
//! - [adaptors] contains all implemented solver adaptors.
//!
//! - The [model_modifier] submodule defines types to help with incremental solving / changing a
//!   model during search. The entrypoint for incremental solving is the [Solver<A,ModelLoaded>::solve_mut]
//!   function.
//!
//! # Examples
//!
//! ## A Successful Minion Model
//!
//! ```rust
//! # use conjure_oxide::generate_custom::get_example_model;
//! use conjure_oxide::rule_engine::rewrite::rewrite_model;
//! use conjure_oxide::rule_engine::resolve_rules::resolve_rule_sets;
//! use conjure_oxide::solver::{Solver,adaptors,SolverAdaptor};
//! use conjure_oxide::solver::states::*;
//! use conjure_oxide::SolverFamily;
//! use std::sync::{Arc,Mutex};
//!
//! // Define and rewrite a model for minion.
//! let model = get_example_model("bool-03").unwrap();
//! let rule_sets = resolve_rule_sets(SolverFamily::Minion, vec!["Constant"]).unwrap();
//! let model = rewrite_model(&model,&rule_sets).unwrap();
//!
//!
//! // Solve using Minion.
//! let solver = Solver::new(adaptors::Minion::new());
//! let solver: Solver<adaptors::Minion,ModelLoaded> = solver.load_model(model).unwrap();
//!
//! // In this example, we will count solutions.
//! //
//! // The solver interface is designed to allow adaptors to use multiple-threads / processes if
//! // necessary. Therefore, the callback type requires all variables inside it to have a static
//! // lifetime and to implement Send (i.e. the variable can be safely shared between theads).
//! //
//! // We use Arc<Mutex<T>> to create multiple references to a threadsafe mutable
//! // variable of type T.
//! //
//! // Using the move |x| ... closure syntax, we move one of these references into the closure.
//! // Note that a normal closure borrow variables from the parent so is not
//! // thread-safe.
//!
//! let counter_ref = Arc::new(Mutex::new(0));
//! let counter_ref_2 = counter_ref.clone();
//! solver.solve(Box::new(move |_| {
//!   let mut counter = (*counter_ref_2).lock().unwrap();
//!   *counter += 1;
//!   true
//!   }));
//!
//! let mut counter = (*counter_ref).lock().unwrap();
//! assert_eq!(*counter,2);
//! ```
//!
//!

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
#![allow(clippy::manual_non_exhaustive)]

use std::collections::HashMap;
use std::error::Error;
use std::fmt::{Debug, Display};
use std::time::Instant;

use thiserror::Error;

use crate::ast::{Constant, Name};
use crate::Model;

use self::model_modifier::*;
use self::states::*;
use self::stats::SolverStats;

pub mod adaptors;
pub mod model_modifier;
pub mod stats;

#[doc(hidden)]
mod private;

pub mod states;

/// The type for user-defined callbacks for use with [Solver].
///
/// Note that this enforces threadsafetyb
pub type SolverCallback = Box<dyn Fn(HashMap<Name, Constant>) -> bool + Send>;
pub type SolverMutCallback<A> =
    Box<dyn Fn(HashMap<Name, Constant>, <A as SolverAdaptor>::Modifier) -> bool + Send>;

/// A common interface for calling underlying solver APIs inside a [`Solver`].
///
/// Implementations of this trait arn't directly callable and should be used through [`Solver`] .
///
/// The below documentation lists the formal requirements that all implementations of
/// [`SolverAdaptor`] should follow - **see the top level module documentation and [`Solver`] for
/// usage details.**
///
/// # Encapsulation
///  
///  The [`SolverAdaptor`] trait **must** only be implemented inside a submodule of this one,
///  and **should** only be called through [`Solver`].
///
/// The `private::Sealed` trait and `private::Internal` type enforce these requirements by only
/// allowing trait implementations and calling of methods of SolverAdaptor to occur inside this
/// module.
///
/// # Thread Safety
///
/// Multiple instances of [`Solver`] can be run in parallel across multiple threads.
///
/// [`Solver`] provides no concurrency control or thread-safety; therefore, adaptors **must**
/// ensure that multiple instances of themselves can be ran in parallel. This applies to all
/// stages of solving including having two active `solve()` calls happening at a time, loading
/// a model while another is mid-solve, loading two models at once, etc.
///
/// A [SolverAdaptor] **may** use whatever threading or process model it likes underneath the hood,
/// as long as it obeys the above.
///
/// Method calls **should** block instead of erroring where possible.
///
/// Underlying solvers that only have one instance per process (such as Minion) **should** block
/// (eg. using a [`Mutex<()>`](`std::sync::Mutex`)) to run calls to
/// [`Solver<A,ModelLoaded>::solve()`] and [`Solver<A,ModelLoaded>::solve_mut()`] sequentially.
pub trait SolverAdaptor: private::Sealed {
    /// The native model type of the underlying solver.
    type Model: Clone;

    /// The native solution type of the underlying solver.
    type Solution: Clone;

    /// The [`ModelModifier`](model_modifier::ModelModifier) used during incremental search.
    ///
    /// If incremental solving is not supported, this **should** be set to [NotModifiable](model_modifier::NotModifiable) .
    type Modifier: model_modifier::ModelModifier;

    fn new() -> Self;

    /// Runs the solver on the given model.
    ///
    /// Implementations of this function **must** call the user provided callback whenever a solution
    /// is found. If the user callback returns `true`, search should continue, if the user callback
    /// returns `false`, search should terminate.
    ///
    /// # Returns
    ///
    /// If the solver terminates without crashing a [SolveSuccess] struct **must** returned. The
    /// value of [SearchStatus] can be used to denote whether the underlying solver completed its
    /// search or not. The latter case covers most non-crashing "failure" cases including user
    /// termination, timeouts, etc.
    ///
    /// To help populate [SearchStatus], it may be helpful to implement counters that track if the
    /// user callback has been called yet, and its return value. This information makes it is
    /// possible to distinguish between the most common search statuses:
    /// [SearchComplete::HasSolutions], [SearchComplete::NoSolutions], and
    /// [SearchIncomplete::UserTerminated].
    fn solve(
        &mut self,
        model: Self::Model,
        callback: SolverCallback,
        _: private::Internal,
    ) -> Result<SolveSuccess, SolverError>;

    /// Runs the solver on the given model, allowing modification of the model through a
    /// [`ModelModifier`].
    ///
    /// Implementations of this function **must** return [`OpNotSupported`](`ModificationFailure::OpNotSupported`)
    /// if modifying the model mid-search is not supported.
    ///
    /// Otherwise, this should work in the same way as [`solve`](SolverAdaptor::solve).
    fn solve_mut(
        &mut self,
        model: Self::Model,
        callback: SolverMutCallback<Self>,
        _: private::Internal,
    ) -> Result<SolveSuccess, SolverError>;
    fn load_model(
        &mut self,
        model: Model,
        _: private::Internal,
    ) -> Result<Self::Model, SolverError>;
    fn init_solver(&mut self, _: private::Internal) {}
}

/// An abstract representation of a constraints solver.
///
/// [Solver] provides a common interface for interacting with a constraint solver. It also
/// abstracts over solver-specific datatypes, handling the translation to/from [conjure_core::ast]
/// types for a model and its solutions.
///
/// Details of how a model is solved is specified by the [SolverAdaptor]. This includes: the
/// underlying solver used, the translation of the model to a solver compatible form, how solutions
/// are translated back to [conjure_core::ast] types, and how incremental solving is implemented.
/// As such, there may be multiple [SolverAdaptor] implementations for a single underlying solver:
/// eg. one adaptor may give solutions in a representation close to the solvers, while another may
/// attempt to rewrite it back into Essence.
///
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
    pub fn load_model(mut self, model: Model) -> Result<Solver<A, ModelLoaded>, SolverError> {
        let solver_model = &mut self.adaptor.load_model(model, private::Internal)?;
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
        callback: SolverCallback,
    ) -> Result<Solver<A, ExecutionSuccess>, SolverError> {
        #[allow(clippy::unwrap_used)]
        let start_time = Instant::now();

        #[allow(clippy::unwrap_used)]
        let result = self
            .adaptor
            .solve(self.model.clone().unwrap(), callback, private::Internal);

        let duration = start_time.elapsed();

        match result {
            Ok(x) => Ok(Solver {
                adaptor: self.adaptor,
                model: self.model,
                state: ExecutionSuccess {
                    stats: x.stats,
                    status: x.status,
                    _sealed: private::Internal,
                    wall_time_s: duration.as_secs_f64(),
                },
            }),
            Err(x) => Err(x),
        }
    }

    pub fn solve_mut(
        mut self,
        callback: SolverMutCallback<A>,
    ) -> Result<Solver<A, ExecutionSuccess>, SolverError> {
        #[allow(clippy::unwrap_used)]
        let start_time = Instant::now();

        #[allow(clippy::unwrap_used)]
        let result =
            self.adaptor
                .solve_mut(self.model.clone().unwrap(), callback, private::Internal);

        let duration = start_time.elapsed();

        match result {
            Ok(x) => Ok(Solver {
                adaptor: self.adaptor,
                model: self.model,
                state: ExecutionSuccess {
                    stats: x.stats,
                    status: x.status,
                    _sealed: private::Internal,
                    wall_time_s: duration.as_secs_f64(),
                },
            }),
            Err(x) => Err(x),
        }
    }
}

impl<A: SolverAdaptor> Solver<A, ExecutionSuccess> {
    pub fn stats(self) -> Option<Box<dyn SolverStats>> {
        self.state.stats
    }

    pub fn wall_time_s(&self) -> f64 {
        self.state.wall_time_s
    }
}

/// Errors returned by [Solver] on failure.
#[non_exhaustive]
#[derive(Debug, Error, Clone)]
pub enum SolverError {
    #[error("operation not implemented yet: {0}")]
    OpNotImplemented(String),

    #[error("operation not supported: {0}")]
    OpNotSupported(String),

    #[error("model feature not supported: {0}")]
    ModelFeatureNotSupported(String),

    #[error("model feature not implemented yet: {0}")]
    ModelFeatureNotImplemented(String),

    // use for semantics / type errors, use the above for syntax
    #[error("model invalid: {0}")]
    ModelInvalid(String),

    #[error("error during solver execution: not implemented: {0}")]
    RuntimeNotImplemented(String),

    #[error("error during solver execution: {0}")]
    Runtime(String),
}

/// Returned from [SolverAdaptor] when solving is successful.
pub struct SolveSuccess {
    stats: Option<Box<dyn SolverStats>>,
    status: SearchStatus,
}

pub enum SearchStatus {
    /// The search was complete (i.e. the solver found all possible solutions)
    Complete(SearchComplete),
    /// The search was incomplete (i.e. it was terminated before all solutions were found)
    Incomplete(SearchIncomplete),
}

#[non_exhaustive]
pub enum SearchIncomplete {
    Timeout,
    UserTerminated,
    #[doc(hidden)]
    /// This variant should not be matched - it exists to simulate non-exhaustiveness of this enum.
    __NonExhaustive,
}

#[non_exhaustive]
pub enum SearchComplete {
    HasSolutions,
    NoSolutions,
    #[doc(hidden)]
    /// This variant should not be matched - it exists to simulate non-exhaustiveness of this enum.
    __NonExhaustive,
}
