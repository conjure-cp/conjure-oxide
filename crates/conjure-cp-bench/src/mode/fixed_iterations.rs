//! A benchmarking mode that loops over each (model,param) combination, running a fixed number of
//! iterations for each.

use std::collections::hash_map::Entry;

use crate::{
    Model, ModelAndParams, ParamFile,
    executor::{Executor, ExecutorOutput},
    mode::{BenchmarkingMode, ModeOutputKey},
};

use super::{ModeError, ModeOutput};

#[derive(Clone, PartialEq, Eq, Debug)]
pub struct FixedIterationsMode {
    iterations: usize,
}

impl FixedIterationsMode {
    pub fn new(iterations: usize) -> FixedIterationsMode {
        FixedIterationsMode { iterations }
    }
}

impl BenchmarkingMode for FixedIterationsMode {
    fn run(
        &self,
        model_and_params: &[ModelAndParams],
        executor: &dyn Executor,
    ) -> Result<ModeOutput, ModeError> {
        let mut output = ModeOutput::new();
        for (model, param_file) in model_and_params.iter().flat_map(|x| x.iter()) {
            for i in 0..self.iterations {
                run_once(&mut output, executor, model, param_file, i, self.iterations)?
            }
        }

        Ok(output)
    }
}

fn run_once(
    output: &mut ModeOutput,
    executor: &dyn Executor,
    model: &Model,
    param_file: Option<&ParamFile>,
    i: usize,
    iterations: usize,
) -> Result<(), ModeError> {
    let executor_result = executor.run(model, param_file);
    let Ok(ExecutorOutput { metadata, counters }) = executor_result else {
        panic!("Todo: propogate executor error")
    };

    // update output.metadata
    for (key, value) in metadata.into_iter() {
        let key = ModeOutputKey {
            model: model.name.clone(),
            param: param_file.map(|x| x.name.clone()),
            counter_name: key,
        };

        match output.executor_metadata.entry(key) {
            Entry::Occupied(mut occupied_entry) => {
                let entry_val = occupied_entry.get_mut();
                entry_val[i] = Some(value);
            }
            Entry::Vacant(vacant_entry) => {
                // when we see a counter we havent seen yet, create a vector of size
                // iterations, so xs[i] = counter value for ith run.
                let entry_val = vacant_entry.insert(vec![None; iterations]);
                entry_val[i] = Some(value);
            }
        };
    }

    // update output.counters
    for (key, value) in counters.into_iter() {
        let key = ModeOutputKey {
            model: model.name.clone(),
            param: param_file.map(|x| x.name.clone()),
            counter_name: key,
        };

        match output.counters.entry(key) {
            Entry::Occupied(mut occupied_entry) => {
                let entry_val = occupied_entry.get_mut();
                entry_val[i] = value;
            }
            Entry::Vacant(vacant_entry) => {
                // when we see a counter we havent seen yet, create a vector of size
                // iterations, so xs[i] = counter value for ith run.
                let entry_val = vacant_entry.insert(vec![None; iterations]);
                entry_val[i] = value;
            }
        };
    }

    Ok(())
}
