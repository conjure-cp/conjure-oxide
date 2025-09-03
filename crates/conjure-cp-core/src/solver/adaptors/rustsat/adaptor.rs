use std::any::type_name;
use std::fmt::format;
use std::hash::Hash;
use std::iter::Inspect;
use std::ops::Deref;
use std::ptr::null;
use std::vec;

use clap::error;
use minion_sys::ast::{Model, Tuple};
use rustsat::encodings::am1::Def;
use rustsat::solvers::{Solve, SolverResult};
use rustsat::types::{Assignment, Clause, Lit, TernaryVal, Var as satVar};
use std::collections::{BTreeMap, HashMap};
use std::result::Result::Ok;
use tracing_subscriber::filter::DynFilterFn;
use ustr::Ustr;

use crate::ast::Domain::Bool;

use rustsat_minisat::core::Minisat;

use crate::ast::Metadata;
use crate::ast::{Atom, Expression, Literal, Name};
use crate::solver::SearchComplete::NoSolutions;
use crate::solver::adaptors::rustsat::convs::handle_cnf;
use crate::solver::{
    self, SearchStatus, SolveSuccess, SolverAdaptor, SolverCallback, SolverError, SolverFamily,
    SolverMutCallback, private,
};
use crate::stats::SolverStats;
use crate::{Model as ConjureModel, ast as conjure_ast, bug};
use crate::{into_matrix_expr, matrix_expr};

use rustsat::instances::{BasicVarManager, Cnf, ManageVars, SatInstance};

use thiserror::Error;

/// A [SolverAdaptor] for interacting with the SatSolver generic and the types thereof.
pub struct Sat {
    __non_constructable: private::Internal,
    model_inst: Option<SatInstance>,
    var_map: Option<HashMap<String, Lit>>,
    solver_inst: Minisat,
    decision_refs: Option<Vec<String>>,
}

impl private::Sealed for Sat {}

impl Default for Sat {
    fn default() -> Self {
        Sat {
            __non_constructable: private::Internal,
            solver_inst: Minisat::default(),
            var_map: None,
            model_inst: None,
            decision_refs: None,
        }
    }
}

fn get_ref_sols(
    find_refs: Vec<String>,
    sol: Assignment,
    var_map: HashMap<String, Lit>,
) -> HashMap<Name, Literal> {
    let mut solution: HashMap<Name, Literal> = HashMap::new();

    for reference in find_refs {
        // lit is 'Nothing' for unconstrained - if this is actually happenning, panicking is fine
        // we are not supposed to do anything to resolve that here.
        let lit: Lit = match var_map.get(&reference) {
            Some(a) => *a,
            None => panic!(
                "There should never be a non-just literal occurring here. Something is broken upstream."
            ),
        };
        solution.insert(
            Name::User(Ustr::from(&reference)),
            match sol[lit.var()] {
                TernaryVal::True => Literal::Int(1),
                TernaryVal::False => Literal::Int(0),
                TernaryVal::DontCare => Literal::Int(2),
            },
        );
    }

    solution
}

impl SolverAdaptor for Sat {
    fn solve(
        &mut self,
        callback: SolverCallback,
        _: private::Internal,
    ) -> Result<SolveSuccess, SolverError> {
        let mut solver = &mut self.solver_inst;

        let cnf: (Cnf, BasicVarManager) = self.model_inst.clone().unwrap().into_cnf();

        (*(solver)).add_cnf(cnf.0);

        let mut has_sol = false;
        loop {
            let res = solver.solve().unwrap();

            match res {
                SolverResult::Sat => {}
                SolverResult::Unsat => {
                    return Ok(SolveSuccess {
                        stats: SolverStats {
                            conjure_solver_wall_time_s: -1.0,
                            solver_family: Some(self.get_family()),
                            solver_adaptor: Some("SAT".to_string()),
                            nodes: None,
                            satisfiable: None,
                            sat_vars: None,
                            sat_clauses: None,
                        },
                        status: if has_sol {
                            SearchStatus::Complete(solver::SearchComplete::HasSolutions)
                        } else {
                            SearchStatus::Complete(NoSolutions)
                        },
                    });
                }
                SolverResult::Interrupted => {
                    return Err(SolverError::Runtime("!!Interrupted Solution!!".to_string()));
                }
            };

            let sol = solver.full_solution().unwrap();
            has_sol = true;
            let solution = get_ref_sols(
                self.decision_refs.clone().unwrap(),
                sol.clone(),
                self.var_map.clone().unwrap(),
            );

            if !callback(solution) {
                // println!("callback false");
                return Ok(SolveSuccess {
                    stats: SolverStats {
                        conjure_solver_wall_time_s: -1.0,
                        solver_family: Some(self.get_family()),
                        solver_adaptor: Some("SAT".to_string()),
                        nodes: None,
                        satisfiable: None,
                        sat_vars: None,
                        sat_clauses: None,
                    },
                    status: SearchStatus::Incomplete(solver::SearchIncomplete::UserTerminated),
                });
            }

            let blocking_vec: Vec<_> = sol.clone().iter().map(|lit| !lit).collect();
            let mut blocking_cl = Clause::new();
            for lit_i in blocking_vec {
                blocking_cl.add(lit_i);
            }

            solver.add_clause(blocking_cl).unwrap();
        }
    }

    fn solve_mut(
        &mut self,
        callback: SolverMutCallback,
        _: private::Internal,
    ) -> Result<SolveSuccess, SolverError> {
        Err(SolverError::OpNotSupported("solve_mut".to_owned()))
    }

    fn load_model(&mut self, model: ConjureModel, _: private::Internal) -> Result<(), SolverError> {
        let sym_tab = model.as_submodel().symbols().deref().clone();
        let decisions = sym_tab.into_iter();

        let mut finds: Vec<String> = Vec::new();

        for find_ref in decisions {
            if (find_ref.1.domain().unwrap() != Bool) {
                Err(SolverError::ModelInvalid(
                    "Only Boolean Decision Variables supported".to_string(),
                ))?;
            }

            let name = find_ref.0;
            finds.push(name.to_string());
        }

        self.decision_refs = Some(finds);

        let m_clone = model;
        let vec_constr = m_clone.as_submodel().constraints();

        let vec_cnf = vec_constr.clone();

        let mut var_map: HashMap<String, Lit> = HashMap::new();

        let inst: SatInstance = handle_cnf(&vec_cnf, &mut var_map);

        self.var_map = Some(var_map);
        let cnf: (Cnf, BasicVarManager) = inst.clone().into_cnf();
        tracing::info!("CNF: {:?}", cnf.0);
        self.model_inst = Some(inst);

        Ok(())
    }

    fn init_solver(&mut self, _: private::Internal) {}

    fn get_family(&self) -> SolverFamily {
        SolverFamily::Sat
    }

    fn get_name(&self) -> Option<String> {
        Some("SAT".to_string())
    }

    fn add_adaptor_info_to_stats(&self, stats: SolverStats) -> SolverStats {
        SolverStats {
            solver_adaptor: self.get_name(),
            solver_family: Some(self.get_family()),
            ..stats
        }
    }

    fn write_solver_input_file(
        &self,
        writer: &mut impl std::io::Write,
    ) -> Result<(), std::io::Error> {
        // TODO: add comments saying what conjure oxide variables each clause has
        // e.g.
        //      c y x z
        //        1 2 3
        //      c x -y
        //        1 -1
        // This will require handwriting a dimacs writer, but that should be easy. For now, just
        // let rustsat write the dimacs.

        let model = self.model_inst.clone().expect("model should exist when we write the solver input file, as we should be in the LoadedModel state");
        let (cnf, var_manager): (Cnf, BasicVarManager) = model.into_cnf();
        cnf.write_dimacs(writer, var_manager.n_used())
    }
}
