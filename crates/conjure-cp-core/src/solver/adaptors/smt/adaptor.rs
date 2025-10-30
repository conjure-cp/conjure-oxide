use z3::Solver;

use super::convert_model::*;
use super::store::*;
use super::theories::*;

use crate::{Model, solver::*};

/// A [SolverAdaptor] for interacting with SMT solvers, specifically Z3.
pub struct Smt {
    __non_constructable: private::Internal,

    /// Initially maps variables to unknown constants.
    /// Also used to store their solved literal values.
    store: Store,

    /// Assertions are added to this solver instance when loading the model.
    solver_inst: Solver,

    theory_config: TheoryConfig,
}

impl private::Sealed for Smt {}

impl Default for Smt {
    fn default() -> Self {
        Smt {
            __non_constructable: private::Internal,
            store: Store::new(),
            solver_inst: Solver::new(),
            theory_config: Default::default(),
        }
    }
}

impl Smt {
    /// Constructs a new adaptor using the given theories for representing the relevant constructs.
    pub fn new(int_theory: IntTheory) -> Self {
        Smt {
            theory_config: TheoryConfig { ints: int_theory },
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
        let solutions = self
            .solver_inst
            .solutions(&self.store, true)
            .take_while(|store| (callback)(store.literals_map().unwrap()));

        // Consume iterator and get whether there are solutions
        let search_complete = match solutions.count() {
            0 => SearchComplete::NoSolutions,
            _ => SearchComplete::HasSolutions,
        };

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
        SolverFamily::Smt
    }

    fn get_name(&self) -> Option<String> {
        Some("SMT".to_string())
    }

    fn write_solver_input_file(
        &self,
        writer: &mut impl std::io::Write,
    ) -> Result<(), std::io::Error> {
        let smt2 = self.solver_inst.to_smt2();
        writer.write(smt2.as_bytes()).map(|_| ())
    }
}
