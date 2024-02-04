#![allow(dead_code)]
#![allow(unused)]

use anyhow::anyhow;
use std::collections::HashMap;
use std::error::Error;
use std::fmt::{Debug, Display};

use conjure_core::ast::{Domain, Expression, Model, Name};

struct Init;
struct HasModel;
struct HasRun;
struct ExecutionSuccess;

enum ExecutionFailure {
    NotImplemented,
    Timeout,
    // What type here??
    Error(String),
}

// TODO: seal trait.
trait SolverState {}
impl SolverState for Init {}
impl SolverState for HasModel {}
impl SolverState for ExecutionSuccess {}
impl SolverState for ExecutionFailure {}

// TODO: this will use constant when it exists
type Callback = fn(bindings: HashMap<String, String>) -> bool;

enum ModificationFailure<E: Error> {
    OperationNotSupported,
    OperationNotImplementedYet,
    ArgsNotSupported(String),
    Error(E),
}

/// A `ModelModifier` allows the modification of a model during solving.
///
/// Modifications are defined in terms of Conjure AST nodes, so must be translated to a solver
/// specfic form before use.
///
/// It is implementation defined whether these constraints can be given at high level and passed
/// through the rewriter, or only low-level solver constraints are supported.
///
/// See also: [`SolverAdaptor::solve_mut`].
trait ModelModifier {
    type Error: Error;
    fn add_constraint(constraint: Expression) -> Result<(), ModificationFailure<Self::Error>> {
        Err(ModificationFailure::OperationNotSupported)
    }

    fn add_variable(name: Name, domain: Domain) -> Result<(), ModificationFailure<Self::Error>> {
        Err(ModificationFailure::OperationNotSupported)
    }
}

/// A [`ModelModifier`] for a solver that does not support incremental solving. Returns
/// [`OperationNotSupported`](`ModificationFailure::OperationNotSupported`) for all operations.
struct NotModifiable;

#[derive(thiserror::Error, Debug)]
enum NotModifiableError {}

impl ModelModifier for NotModifiable {
    type Error = NotModifiableError;
}

// TODO: seal trait?
trait SolverAdaptor {
    type Model: Clone;
    type Solution;
    type ExecutionError: Error;
    type TranslationError<'a>: Error + Display + Send + Sync
    where
        Self: 'a;

    /// The [`ModelModifier`] to use during incremental search.
    ///
    /// If incremental solving is not supported, set this to [`NotModifiable`] .
    type Modifier: ModelModifier;

    // TODO: this should be able to go to multiple states.
    // Adaptor implementers must call the user provided callback whenever a solution is found.

    /// Run the solver on the given model.
    ///
    /// Implementations of this function MUST call the user provided callback whenever a solution
    /// is found. If the user callback returns `true`, search should continue, if the user callback
    /// returns `false`, search should terminate.
    fn solve(
        &mut self,
        model: Self::Model,
        callback: Callback,
    ) -> Result<ExecutionSuccess, ExecutionFailure>;

    /// Run the solver on the given model, allowing modification of the model through a
    /// [`ModelModifier`].
    ///
    /// Implementations of this function MUST return [`OperationNotSupported`](`ModificationFailure::OperationNotSupported`)
    /// if modifying the model mid-search is not supported. These implementations may also find the
    /// [`NotModifiable`] modifier useful.
    ///
    /// As with [`solve`](SolverAdaptor::solve), this function MUST call the user provided callback
    /// function whenever a solution is found.
    fn solve_mut(
        &mut self,
        model: Self::Model,
        callback: fn(HashMap<String, String>, Self::Modifier) -> bool,
    ) -> Result<ExecutionSuccess, ExecutionFailure>;
    fn load_model(&mut self, model: Model) -> Result<Self::Model, Self::TranslationError<'_>>;
    fn init_solver(&mut self) {}
}

struct Solver<A: SolverAdaptor, State: SolverState = Init> {
    state: std::marker::PhantomData<State>,
    adaptor: A,
    model: Option<A::Model>,
}

impl<A: SolverAdaptor> Solver<A, Init> {
    // TODO: decent error handling
    pub fn load_model(mut self, model: Model) -> Result<Solver<A, HasModel>, ()> {
        let solver_model = &mut self.adaptor.load_model(model).map_err(|_| ())?;
        Ok(Solver {
            state: std::marker::PhantomData::<HasModel>,
            adaptor: self.adaptor,
            model: Some(solver_model.clone()),
        })
    }
}

impl<A: SolverAdaptor> Solver<A, HasModel> {
    pub fn solve(mut self, callback: Callback) -> Result<ExecutionSuccess, ExecutionFailure> {
        #[allow(clippy::unwrap_used)]
        self.adaptor.solve(self.model.unwrap(), callback)
    }

    pub fn solve_mut(
        mut self,
        callback: fn(HashMap<String, String>, A::Modifier) -> bool,
    ) -> Result<ExecutionSuccess, ExecutionFailure> {
        #[allow(clippy::unwrap_used)]
        self.adaptor.solve_mut(self.model.unwrap(), callback)
    }
}

impl<T: SolverAdaptor> Solver<T> {
    pub fn new(solver_adaptor: T) -> Solver<T> {
        let mut solver = Solver {
            state: std::marker::PhantomData::<Init>,
            adaptor: solver_adaptor,
            model: None,
        };

        solver.adaptor.init_solver();
        solver
    }
}
