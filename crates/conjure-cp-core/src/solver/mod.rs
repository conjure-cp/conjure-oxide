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
//!   between the [Solver] type and a specific solver.
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
//! Note: this example constructs a basic Minion-compatible model instead of using the rewriter.
//! For a full end-to-end example, see crates/conjure-cp/examples/solver_hello_minion.rs
//!
//! ```ignore
//! use std::sync::{Arc,Mutex};
//! use conjure_cp_core::parse::get_example_model;
//! use conjure_cp_core::rule_engine::resolve_rule_sets;
//! use conjure_cp_core::rule_engine::rewrite_naive;
//! use conjure_cp_core::solver::{adaptors, Solver, SolverAdaptor};
//! use conjure_cp_core::solver::states::ModelLoaded;
//! use conjure_cp_core::Model;
//! use conjure_cp_core::ast::Domain;
//! use conjure_cp_core::ast::Declaration;
//! use conjure_cp_core::solver::SolverFamily;
//! use conjure_cp_core::context::Context;
//! use conjure_cp_essence_macros::essence_expr;
//!
//! // Define a model for minion.
//! let context = Context::<'static>::new_ptr_empty(SolverFamily::Minion);
//! let mut model = Model::new(context);
//! model.as_submodel_mut().add_symbol(Declaration::new_var("x".into(), Domain::Bool));
//! model.as_submodel_mut().add_symbol(Declaration::new_var("y".into(), Domain::Bool));
//! model.as_submodel_mut().add_constraint(essence_expr!{x != y});
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
//! # The Solver callback function
//!
//! The callback function given to `solve` is called whenever a solution is found by the solver.
//!
//! Its return value can be used to control how many solutions the solver finds:
//!
//! * If the callback function returns `true`, solver execution continues.
//! * If the callback function returns `false`, the solver is terminated.
//!

// # Implementing Solver interfaces
//
// Solver interfaces can only be implemented inside this module, due to the SolverAdaptor crate
// being sealed.
//
// To add support for a solver, implement the `SolverAdaptor` trait in a submodule.
//
// If incremental solving support is required, also implement a new `ModelModifier`. If this is not
// required, all `ModelModifier` instances required by the SolverAdaptor trait can be replaced with
// NotModifiable.
//
// For more details, see the docstrings for SolverAdaptor, ModelModifier, and NotModifiable.

#![allow(dead_code)]
#![allow(unused)]
#![allow(clippy::manual_non_exhaustive)]

use std::any::Any;
use std::cell::OnceCell;
use std::collections::HashMap;
use std::error::Error;
use std::fmt::{Debug, Display};
use std::io::Write;
use std::rc::Rc;
use std::sync::{Arc, RwLock};
use std::time::Instant;

use clap::ValueEnum;
use schemars::JsonSchema;
use serde::{Deserialize, Serialize};
use strum_macros::{Display, EnumIter, EnumString};
use thiserror::Error;

use crate::Model;
use crate::ast::{Literal, Name};
use crate::context::Context;
use crate::stats::SolverStats;

use self::model_modifier::ModelModifier;
use self::states::{ExecutionSuccess, Init, ModelLoaded, SolverState};

pub mod adaptors;
pub mod model_modifier;

#[doc(hidden)]
mod private;

pub mod states;

#[derive(
    Debug,
    EnumString,
    EnumIter,
    Display,
    PartialEq,
    Eq,
    Hash,
    Clone,
    Copy,
    Serialize,
    Deserialize,
    JsonSchema,
    ValueEnum,
)]
pub enum SolverFamily {
    Sat,
    Minion,
}

/// The type for user-defined callbacks for use with [Solver].
///
/// Note that this enforces thread safety
pub type SolverCallback = Box<dyn Fn(HashMap<Name, Literal>) -> bool + Send>;
pub type SolverMutCallback =
    Box<dyn Fn(HashMap<Name, Literal>, Box<dyn ModelModifier>) -> bool + Send>;

/// A common interface for calling underlying solver APIs inside a [`Solver`].
///
/// Implementations of this trait aren't directly callable and should be used through [`Solver`] .
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
pub trait SolverAdaptor: private::Sealed + Any {
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
        callback: SolverMutCallback,
        _: private::Internal,
    ) -> Result<SolveSuccess, SolverError>;
    fn load_model(&mut self, model: Model, _: private::Internal) -> Result<(), SolverError>;
    fn init_solver(&mut self, _: private::Internal) {}

    /// Get the solver family that this solver adaptor belongs to
    fn get_family(&self) -> SolverFamily;

    /// Gets the name of the solver adaptor for pretty printing.
    fn get_name(&self) -> Option<String> {
        None
    }

    /// Adds the solver adaptor name and family (if they exist) to the given stats object.
    fn add_adaptor_info_to_stats(&self, stats: SolverStats) -> SolverStats {
        SolverStats {
            solver_adaptor: self.get_name(),
            solver_family: Some(self.get_family()),
            ..stats
        }
    }

    /// Writes a solver input file to the given writer.
    ///
    /// This method is for debugging use only, and there are no plans to make the solutions
    /// obtained by running this file through the solver translatable back into high-level Essence.
    ///
    /// This file is runnable using the solvers command line interface. E.g. for Minion, this
    /// outputs a valid .minion file.
    ///
    ///
    /// # Implementation
    /// + It can be helpful for this file to contain comments linking constraints and variables to
    ///   their original essence, but this is not required.
    ///
    /// + This function is ran after model loading but before solving - therefore, it is safe for
    ///   solving to mutate the model object.
    fn write_solver_input_file(&self, writer: &mut impl Write) -> Result<(), std::io::Error>;
}

/// An abstract representation of a constraints solver.
///
/// [Solver] provides a common interface for interacting with a constraint solver. It also
/// abstracts over solver-specific datatypes, handling the translation to/from [conjure_cp_core::ast]
/// types for a model and its solutions.
///
/// Details of how a model is solved is specified by the [SolverAdaptor]. This includes: the
/// underlying solver used, the translation of the model to a solver compatible form, how solutions
/// are translated back to [conjure_cp_core::ast] types, and how incremental solving is implemented.
/// As such, there may be multiple [SolverAdaptor] implementations for a single underlying solver:
/// e.g. one adaptor may give solutions in a representation close to the solvers, while another may
/// attempt to rewrite it back into Essence.
///
#[derive(Clone)]
pub struct Solver<A: SolverAdaptor, State: SolverState = Init> {
    state: State,
    adaptor: A,
    context: Option<Arc<RwLock<Context<'static>>>>,
}

impl<Adaptor: SolverAdaptor> Solver<Adaptor> {
    pub fn new(solver_adaptor: Adaptor) -> Solver<Adaptor> {
        let mut solver = Solver {
            state: Init,
            adaptor: solver_adaptor,
            context: None,
        };

        solver.adaptor.init_solver(private::Internal);
        solver
    }

    pub fn get_family(&self) -> SolverFamily {
        self.adaptor.get_family()
    }
}

impl<A: SolverAdaptor> Solver<A, Init> {
    pub fn load_model(mut self, model: Model) -> Result<Solver<A, ModelLoaded>, SolverError> {
        let solver_model = &mut self.adaptor.load_model(model.clone(), private::Internal)?;
        Ok(Solver {
            state: ModelLoaded,
            adaptor: self.adaptor,
            context: Some(model.context.clone()),
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
        let result = self.adaptor.solve(callback, private::Internal);

        let duration = start_time.elapsed();

        match result {
            Ok(x) => {
                let stats = self
                    .adaptor
                    .add_adaptor_info_to_stats(x.stats)
                    .with_timings(duration.as_secs_f64());

                Ok(Solver {
                    adaptor: self.adaptor,
                    state: ExecutionSuccess {
                        stats,
                        status: x.status,
                        _sealed: private::Internal,
                    },
                    context: self.context,
                })
            }
            Err(x) => Err(x),
        }
    }

    pub fn solve_mut(
        mut self,
        callback: SolverMutCallback,
    ) -> Result<Solver<A, ExecutionSuccess>, SolverError> {
        #[allow(clippy::unwrap_used)]
        let start_time = Instant::now();

        #[allow(clippy::unwrap_used)]
        let result = self.adaptor.solve_mut(callback, private::Internal);

        let duration = start_time.elapsed();

        match result {
            Ok(x) => {
                let stats = self
                    .adaptor
                    .add_adaptor_info_to_stats(x.stats)
                    .with_timings(duration.as_secs_f64());

                Ok(Solver {
                    adaptor: self.adaptor,
                    state: ExecutionSuccess {
                        stats,
                        status: x.status,
                        _sealed: private::Internal,
                    },
                    context: self.context,
                })
            }
            Err(x) => Err(x),
        }
    }

    /// Writes a solver input file to the given writer.
    ///
    /// This method is for debugging use only, and there are no plans to make the solutions
    /// obtained by running this file through the solver translatable back into high-level Essence.
    ///
    /// This file is runnable using the solvers command line interface. E.g. for Minion, this
    /// outputs a valid .minion file.
    ///
    /// This function is only available in the `ModelLoaded` state as solvers are allowed to edit
    /// the model in place.
    pub fn write_solver_input_file(&self, writer: &mut impl Write) -> Result<(), std::io::Error> {
        self.adaptor.write_solver_input_file(writer)
    }
}

impl<A: SolverAdaptor> Solver<A, ExecutionSuccess> {
    pub fn stats(&self) -> SolverStats {
        self.state.stats.clone()
    }

    // Saves this solvers stats to the global context as a "solver run"
    pub fn save_stats_to_context(&self) {
        #[allow(clippy::unwrap_used)]
        #[allow(clippy::expect_used)]
        self.context
            .as_ref()
            .expect("")
            .write()
            .unwrap()
            .stats
            .add_solver_run(self.stats());
    }

    pub fn wall_time_s(&self) -> f64 {
        self.stats().conjure_solver_wall_time_s
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
    stats: SolverStats,
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
