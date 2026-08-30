use std::collections::HashMap;

use thiserror::Error;

use crate::{Model, ParamFile};

pub mod conjure;

pub trait Executor {
    fn run(
        &self,
        model: &Model,
        param_file: Option<&ParamFile>,
    ) -> Result<ExecutorOutput, ExecutorError>;
}

#[derive(Clone, Debug, PartialEq)]
#[non_exhaustive]
pub struct ExecutorOutput {
    pub metadata: HashMap<String, String>,
    pub counters: HashMap<String, Option<f64>>,
}

impl Default for ExecutorOutput {
    fn default() -> Self {
        Self::new()
    }
}

impl ExecutorOutput {
    pub fn new() -> Self {
        ExecutorOutput {
            metadata: HashMap::new(),
            counters: HashMap::new(),
        }
    }

    pub(crate) fn set_metadata(&mut self, key: &str, value: String) {
        self.metadata.insert(key.to_string(), value);
    }

    pub(crate) fn set_counter(&mut self, key: &str, value: Option<f64>) {
        self.counters.insert(key.to_string(), value);
    }
}

#[derive(Clone, Debug, PartialEq, Eq, Error)]
#[non_exhaustive]
pub enum ExecutorError {}
