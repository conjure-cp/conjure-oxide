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

use crate::ast::Domain::{Bool, Int};

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

use itertools::Itertools;
/// A [SolverAdaptor] for interacting with the SatSolver generic and the types thereof.
pub struct Sat {
    __non_constructable: private::Internal,
    model_inst: Option<SatInstance>,
    var_map: Option<HashMap<Name, Lit>>,
    solver_inst: Minisat,
    decision_refs: Option<Vec<Name>>,
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
    find_refs: Vec<Name>,
    sol: Assignment,
    var_map: HashMap<Name, Lit>,
) -> HashMap<Name, Literal> {
    let mut solution: HashMap<Name, Literal> = HashMap::new();

    for reference in find_refs {
        // lit is `Nothing` for variables that don't exist. This should have thrown an error at parse-time.
        let lit: Lit = match var_map.get(&reference) {
            Some(a) => *a,
            None => bug!(
                "There should never be a non-just literal occurring here. Something is broken upstream."
            ),
        };

        solution.insert(
            reference,
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

            let mut sol = solver.full_solution().unwrap();

            // add DontCares into the solution
            for (name, lit) in self.var_map.clone().unwrap() {
                let inserter = sol.var_value(lit.var());
                sol.assign_var(lit.var(), inserter);
            }
            has_sol = true;
            let sol_old = get_ref_sols(
                self.decision_refs.clone().unwrap(),
                sol.clone(),
                self.var_map.clone().unwrap(),
            );

            tracing::info!("old solution {:#?}", sol_old);

            let solutions = enumerate_all_solutions(sol_old);

            tracing::info!("final solutions for run");
            tracing::info!("{:#?}", solutions);

            for solution in solutions {
                if !callback(solution) {
                    // callback false
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
            }

            let blocking_vec: Vec<_> = sol.clone().iter().map(|lit| !lit).collect();
            let mut blocking_cl = Clause::new();
            tracing::info!("adding blocking clause with literals: {:#?}", blocking_vec);
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
        let decisions = sym_tab.clone().into_iter();

        let mut finds: Vec<Name> = Vec::new();
        let mut var_map: HashMap<Name, Lit> = HashMap::new();

        for find_ref in decisions {
            let domain = find_ref.1.domain().unwrap();

            // only decision variables with boolean domains or representations using booleans are supported at this time
            if (domain != Bool
                && (sym_tab
                    .get_representation(&find_ref.0, &["sat_log_int"])
                    .is_none()))
            {
                Err(SolverError::ModelInvalid(
                    "Only Boolean Decision Variables supported".to_string(),
                ))?;
            }
            // only boolean variables should be passed to the solver
            if (domain == Bool) {
                let name = find_ref.0;
                finds.push(name);
            }
        }

        self.decision_refs = Some(finds.clone());

        let m_clone = model;

        let vec_constr = m_clone.as_submodel().clauses();

        let inst: SatInstance = handle_cnf(vec_constr, &mut var_map, finds.clone());

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

// Function that takes in solutions and returns updated solutions
// update consists of checking each assignment and calling enumerate_real if dont-care types exist
fn enumerate_all_solutions(solution: HashMap<Name, Literal>) -> Vec<HashMap<Name, Literal>> {
    tracing::info!("Enumerating");
    for (key, val) in solution.clone() {
        match val {
            Literal::Int(a) => {
                if (a == 2) {
                    return enumerate_solution(solution);
                } else {
                    continue;
                }
            }
            _ => continue,
        }
    }
    vec![solution]
}

// Function that takes in ONE solution and
// unfolds dont-care type ternaries into each possible type
// returns all possible 'real' solutions for this 'generated' solution
// a real solution is one with no variable assigned to Ternary::DontCare
fn enumerate_solution(solution: HashMap<Name, Literal>) -> Vec<HashMap<Name, Literal>> {
    tracing::info!("Enumerating: Real");
    let mut sols = Vec::new();
    let mut dont_cares = Vec::new();
    let mut solutions_inclusive = HashMap::new();

    for (key, val) in solution {
        let v = match val {
            Literal::Int(i) => i,
            _ => bug!("Only Integers expected at this time"),
        };
        if v == 2 {
            // anytime the value is 2 (dont-care in the ternary system used by rustsat), add the
            // key to a vector of dontcare values
            dont_cares.push(key);
        } else {
            // if the value is not a dont-care, then this (k, v) pair is usable in the final
            // solution, so just add it to the inclusive solution (another HashMap)
            solutions_inclusive.insert(key, val);
        }
    }

    let mut tdcs = Vec::new();

    tdcs.push(vec![]);
    for len in 1..(dont_cares.len()) {
        for combination in dont_cares.iter().combinations(len) {
            tdcs.push(combination);
        }
    }

    for trues in tdcs {
        let mut d = solutions_inclusive.clone();
        for key in dont_cares.clone() {
            if trues.contains(&&key) {
                d.insert(key, Literal::Int(1));
            } else {
                d.insert(key, Literal::Int(0));
            }
        }
        sols.push(d);
    }

    for i in dont_cares {
        solutions_inclusive.insert(i, Literal::Int(1));
    }

    sols.push(solutions_inclusive);
    sols
}
