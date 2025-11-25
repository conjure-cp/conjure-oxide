//! States of a [`Solver`].
use std::fmt::Display;

use crate::stats::SolverStats;

use super::SearchStatus;
use super::Solver;
use super::SolverError;
use super::private::Internal;
use super::private::Sealed;

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
    pub stats: SolverStats,

    /// The status of the search
    pub status: SearchStatus,

    /// Cannot construct this from outside this module.
    pub _sealed: Internal,
}

/// The state returned by [`Solver`] if solving has not been successful.
pub struct ExecutionFailure {
    pub why: SolverError,

    /// Cannot construct this from outside this module.
    pub _sealed: Internal,
}
