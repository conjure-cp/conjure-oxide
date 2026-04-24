use regex::Regex;
use std::collections::{HashMap, HashSet};
use std::sync::LazyLock;
use ustr::Ustr;

use minion_ast::Model as MinionModel;
use minion_sys::ast as minion_ast;
use minion_sys::run_minion;

use crate::Model as ConjureModel;
use crate::ast::{self as conjure_ast, Expression, Name};
use crate::settings::SolverFamily;
use crate::solver::SolverCallback;
use crate::solver::SolverMutCallback;
use crate::stats::SolverStats;

use crate::solver::SearchComplete::{HasSolutions, NoSolutions};
use crate::solver::SearchIncomplete::UserTerminated;
use crate::solver::SearchStatus::{Complete, Incomplete};
use crate::solver::SolveSuccess;
use crate::solver::SolverAdaptor;
use crate::solver::SolverError;
use crate::solver::SolverError::OpNotImplemented;
use crate::solver::private;

use super::dominance_injection::{
    add_dominance_constraints_for_solution, add_represented_decision_values,
    minion_error_to_solver_error,
};
use super::parse_model::model_to_minion;

/// A [SolverAdaptor] for interacting with Minion.
///
/// This adaptor uses the `minion_sys` crate to talk to Minion over FFI.
pub struct Minion {
    __non_constructable: private::Internal,
    model: Option<MinionModel>,
    dominance_expression: Option<Expression>,
    dominance_model_template: Option<ConjureModel>,
}

fn parse_name(minion_name: &str) -> Name {
    static MACHINE_NAME_RE: LazyLock<Regex> =
        LazyLock::new(|| Regex::new(r"__conjure_machine_name_([0-9]+)").unwrap());
    static REPRESENTED_NAME_RE: LazyLock<Regex> =
        LazyLock::new(|| Regex::new(r"__conjure_represented_name__(.*)__(.*)___(.*)").unwrap());

    if let Some(caps) = MACHINE_NAME_RE.captures(minion_name) {
        conjure_ast::Name::Machine(caps[1].parse::<i32>().unwrap())
    } else if let Some(caps) = REPRESENTED_NAME_RE.captures(minion_name) {
        conjure_ast::Name::Represented(Box::new((
            parse_name(&caps[1]),
            Ustr::from(&caps[2]),
            Ustr::from(&caps[3]),
        )))
    } else {
        conjure_ast::Name::User(Ustr::from(minion_name))
    }
}

fn translate_solution(
    solutions: HashMap<minion_ast::VarName, minion_ast::Constant>,
) -> HashMap<conjure_ast::Name, conjure_ast::Literal> {
    let mut conjure_solutions: HashMap<conjure_ast::Name, conjure_ast::Literal> = HashMap::new();
    for (minion_name, minion_const) in solutions.into_iter() {
        let conjure_const = match minion_const {
            minion_ast::Constant::Bool(x) => conjure_ast::Literal::Bool(x),
            minion_ast::Constant::Integer(x) => conjure_ast::Literal::Int(x),
            _ => todo!(),
        };

        let conjure_name = parse_name(&minion_name);
        conjure_solutions.insert(conjure_name, conjure_const);
    }
    conjure_solutions
}

impl private::Sealed for Minion {}

impl Minion {
    pub fn new() -> Minion {
        Minion {
            __non_constructable: private::Internal,
            model: None,
            dominance_expression: None,
            dominance_model_template: None,
        }
    }
}

impl Default for Minion {
    fn default() -> Self {
        Minion::new()
    }
}

impl SolverAdaptor for Minion {
    fn solve(
        &mut self,
        callback: SolverCallback,
        _: private::Internal,
    ) -> Result<SolveSuccess, SolverError> {
        let mut any_solutions = false;
        let mut user_terminated = false;
        let dominance_expression = self.dominance_expression.clone();
        let dominance_model_template = self.dominance_model_template.clone();
        let mut midsearch_error: Option<SolverError> = None;
        let base_model = self.model.as_ref().expect("STATE MACHINE ERR");
        let mut known_var_names = base_model
            .named_variables
            .get_variable_order()
            .into_iter()
            .collect::<HashSet<_>>();
        let mut next_midsearch_aux_var_id = 0usize;
        let mut solution_ordinal = 0usize;

        let solver_ctx = run_minion(
            self.model.clone().expect("STATE MACHINE ERR"),
            Box::new(|solutions| {
                any_solutions = true;
                solution_ordinal += 1;
                let mut conjure_solutions = translate_solution(solutions);
                if let Some(model_template) = dominance_model_template.as_ref() {
                    add_represented_decision_values(&mut conjure_solutions, model_template);
                }

                let continue_search = callback(conjure_solutions.clone());
                if !continue_search {
                    user_terminated = true;
                    return false;
                }

                if let Err(err) = add_dominance_constraints_for_solution(
                    dominance_expression.as_ref(),
                    dominance_model_template.as_ref(),
                    &conjure_solutions,
                    &mut known_var_names,
                    &mut next_midsearch_aux_var_id,
                    solution_ordinal,
                ) {
                    midsearch_error = Some(err);
                    return false;
                }

                true
            }),
        )
        .map_err(minion_error_to_solver_error)?;

        if let Some(err) = midsearch_error {
            return Err(err);
        }

        let status = if user_terminated {
            Incomplete(UserTerminated)
        } else if any_solutions {
            Complete(HasSolutions)
        } else {
            Complete(NoSolutions)
        };
        Ok(SolveSuccess {
            stats: get_solver_stats(&solver_ctx),
            status,
        })
    }

    fn solve_mut(
        &mut self,
        callback: SolverMutCallback,
        _: private::Internal,
    ) -> Result<SolveSuccess, SolverError> {
        Err(OpNotImplemented("solve_mut".into()))
    }

    fn load_model(&mut self, model: ConjureModel, _: private::Internal) -> Result<(), SolverError> {
        self.dominance_expression = model.dominance.as_ref().map(|expr| match expr {
            Expression::DominanceRelation(_, inner) => inner.as_ref().clone(),
            _ => expr.clone(),
        });
        self.dominance_model_template = self.dominance_expression.as_ref().map(|_| model.clone());
        self.model = Some(model_to_minion(model)?);
        Ok(())
    }

    fn get_family(&self) -> SolverFamily {
        SolverFamily::Minion
    }

    fn get_name(&self) -> &'static str {
        "minion"
    }

    fn write_solver_input_file(
        &self,
        writer: &mut Box<dyn std::io::Write>,
    ) -> Result<(), std::io::Error> {
        let model = self.model.as_ref().expect("Minion solver adaptor should have a model as write_solver_input_file should only be called in the LoadedModel state.");
        minion_sys::print::write_minion_file(writer, model)
    }
}

#[allow(clippy::unwrap_used)]
fn get_solver_stats(solver_ctx: &minion_sys::SolverContext) -> SolverStats {
    SolverStats {
        nodes: solver_ctx
            .get_from_table("Nodes".into())
            .map(|x| x.parse::<u64>().unwrap()),
        ..Default::default()
    }
}
