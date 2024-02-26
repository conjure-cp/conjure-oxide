//! States of a [`Solver`].
use std::fmt::Display;

use thiserror::Error;

use super::private::Internal;
use super::private::Sealed;
use super::stats::*;
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
pub struct ExecutionSuccess {
    /// Execution statistics.
    pub stats: Box<dyn Stats>,

    // make this struct unconstructable outside of this module
    #[doc(hidden)]
    _private: Internal,
}

/// The state returned by [`Solver`] if solving has not been successful.
#[non_exhaustive]
#[derive(Debug, Error)]
pub enum ExecutionFailure {
    #[error("operation not implemented yet")]
    OpNotImplemented,

    #[error("operation is not supported by this solver")]
    OpNotSupported,

    #[error("time out")]
    TimedOut,

    #[error(transparent)]
    Other(#[from] anyhow::Error),
}
