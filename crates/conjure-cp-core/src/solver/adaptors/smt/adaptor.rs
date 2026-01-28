use std::iter::FusedIterator;
use std::sync::Mutex;

use z3::{
    Config, PrepareSynchronized, SatResult, Solvable, Solver, Statistics, Translate, with_z3_config,
};

use super::convert_model::*;
use super::store::*;
use super::theories::*;

use crate::{Model, solver::*};

/// A [SolverAdaptor] for interacting with SMT solvers, specifically Z3.
pub struct Smt {
    __non_constructable: private::Internal,

    /// Initially maps variables to unknown constants.
    /// Also used to store their solved literal values.
    store: SymbolStore,

    /// Assertions are added to this solver instance when loading the model.
    solver_inst: Solver,

    solver_cfg: Config,

    theory_config: TheoryConfig,
}

impl private::Sealed for Smt {}

impl Default for Smt {
    fn default() -> Self {
        Smt {
            __non_constructable: private::Internal,
            store: SymbolStore::new(TheoryConfig::default()),
            solver_inst: Solver::new(),
            solver_cfg: Config::new(),
            theory_config: TheoryConfig::default(),
        }
    }
}

impl Smt {
    /// Constructs a new adaptor using the given theories for representing the relevant constructs.
    pub fn new(timeout_msec: Option<u64>, theory_config: TheoryConfig) -> Self {
        let mut solver_cfg = Config::new();
        timeout_msec.inspect(|ms| solver_cfg.set_timeout_msec(*ms));

        Smt {
            theory_config,
            solver_cfg,
            store: SymbolStore::new(theory_config),
            ..Default::default()
        }
    }
}

impl SolverAdaptor for Smt {
    fn solve(
        &mut self,
        callback: SolverCallback,
        _: private::Internal,
    ) -> Result<SolveSuccess, SolverError> {
        let solver_send = self.solver_inst.synchronized();
        let store_send = self.store.synchronized();
        let mut stats: SolverStats = Default::default();

        // Apply config when getting solutions
        let (search_complete, final_z3_time) = with_z3_config(&self.solver_cfg, move || {
            let solver = solver_send.recover();
            let mut final_z3_time: Option<f64> = None;

            let solutions = solver
                .into_solutions_with_statistics(store_send.recover(), true)
                .take_while(|(store, z3_stats)| {
                    let time = z3_stats.value("time");
                    if let Some(z3::StatisticsValue::Double(time)) = time {
                        final_z3_time = Some(time);
                    }
                    (callback)(store.as_literals_map().unwrap())
                });

            // Consume iterator and get whether there are solutions
            let search_complete = match solutions.count() {
                0 => SearchComplete::NoSolutions,
                _ => SearchComplete::HasSolutions,
            };
            (search_complete, final_z3_time)
        });

        if let Some(time) = final_z3_time {
            stats.solver_time_s = time;
        }

        Ok(SolveSuccess {
            stats,
            status: SearchStatus::Complete(search_complete),
        })
    }

    fn solve_mut(
        &mut self,
        callback: SolverMutCallback,
        _: private::Internal,
    ) -> Result<SolveSuccess, SolverError> {
        Err(SolverError::OpNotImplemented("solve_mut".into()))
    }

    fn load_model(&mut self, model: Model, _: private::Internal) -> Result<(), SolverError> {
        let submodel = model.as_submodel();
        load_model_impl(
            &mut self.store,
            &mut self.solver_inst,
            &self.theory_config,
            &submodel.symbols(),
            submodel.constraints().as_slice(),
        )?;
        Ok(())
    }

    fn get_family(&self) -> SolverFamily {
        SolverFamily::Smt(self.theory_config)
    }

    fn get_name(&self) -> &'static str {
        "SMT"
    }

    fn write_solver_input_file(
        &self,
        writer: &mut Box<dyn std::io::Write>,
    ) -> Result<(), std::io::Error> {
        let smt2 = self.solver_inst.to_smt2();
        writer.write(smt2.as_bytes()).map(|_| ())
    }
}

trait IntoSolutionsWithStatistics {
    fn into_solutions_with_statistics<T: Solvable>(
        self,
        t: T,
        model_completion: bool,
    ) -> impl FusedIterator<Item = (T::ModelInstance, Statistics)>;
}

impl IntoSolutionsWithStatistics for z3::Solver {
    fn into_solutions_with_statistics<T: Solvable>(
        self,
        t: T,
        model_completion: bool,
    ) -> impl FusedIterator<Item = (T::ModelInstance, Statistics)> {
        SolverStatsIterator {
            solver: self,
            ast: t,
            model_completion,
        }
        .fuse()
    }
}

struct SolverStatsIterator<T> {
    solver: Solver,
    ast: T,
    model_completion: bool,
}

// copy-pasted from upstream except this runs get_statistics
impl<T: Solvable> Iterator for SolverStatsIterator<T> {
    type Item = (T::ModelInstance, Statistics);

    fn next(&mut self) -> Option<Self::Item> {
        match self.solver.check() {
            SatResult::Sat => {
                // right after we solve, before we generate the model grab statistics
                let stats = self.solver.get_statistics();
                let model = self.solver.get_model()?;
                let instance = self.ast.read_from_model(&model, self.model_completion)?;
                let counterexample = self.ast.generate_constraint(&instance);
                self.solver.assert(counterexample);
                // and return them through the iterator
                Some((instance, stats))
            }
            _ => None,
        }
    }
}
