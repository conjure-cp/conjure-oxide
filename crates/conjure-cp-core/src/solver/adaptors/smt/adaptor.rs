use std::sync::Mutex;

use z3::{Config, PrepareSynchronized, Solver, Translate, with_z3_config};

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

        // Apply config when getting solutions
        let search_complete = with_z3_config(&self.solver_cfg, move || {
            let solver = solver_send.recover();
            let solutions = solver
                .solutions(store_send.recover(), true)
                .take_while(|store| (callback)(store.as_literals_map().unwrap()));

            // Consume iterator and get whether there are solutions
            match solutions.count() {
                0 => SearchComplete::NoSolutions,
                _ => SearchComplete::HasSolutions,
            }
        });

        Ok(SolveSuccess {
            // TODO: get solver stats
            stats: Default::default(),
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
