//! Benchmarking modes.
//!
//! A benchmarking mode provides a strategy for running an executor on a set of models and parameter
//! files, possibly for multiple replications.
use crate::{ModelAndParams, executor::Executor};
use std::collections::HashMap;
use thiserror::Error;

mod fixed_iterations;
pub use fixed_iterations::*;
/// A benchmarking mode provides a strategy for running an executor on a set of models and parameter
/// files, possibly for multiple replications.
pub trait BenchmarkingMode {
    fn run(
        &self,
        models_and_params: &[ModelAndParams],
        executor: &dyn Executor,
    ) -> Result<ModeOutput, ModeError>;
}

#[derive(Clone, Debug, PartialEq)]
#[non_exhaustive]
/// Results obtained out of a [benchmarking mode](BenchmarkingMode).
pub struct ModeOutput {
    /// Metadata for the scheduler.
    pub mode_metadata: HashMap<String, String>,

    /// Metadata for each iteration of the program under test, group by model, param file, and
    /// metadata field name.
    pub executor_metadata: HashMap<ModeOutputKey, Vec<Option<String>>>,

    /// Counter values for each iteration of the program under test, grouped by model, param file,
    /// and counter name.
    pub counters: HashMap<ModeOutputKey, Vec<Option<f64>>>,
}

impl ModeOutput {
    pub(crate) fn new() -> ModeOutput {
        ModeOutput {
            mode_metadata: HashMap::new(),
            executor_metadata: HashMap::new(),
            counters: HashMap::new(),
        }
    }
}

#[derive(Clone, Debug, PartialEq, Eq, PartialOrd, Ord, Hash)]
pub struct ModeOutputKey {
    pub model: String,
    pub param: Option<String>,
    pub counter_name: String,
}

#[derive(Clone, Debug, PartialEq, Eq, Error)]
#[non_exhaustive]
pub enum ModeError {}
