use regex::Regex;
use std::collections::HashMap;
use std::sync::LazyLock;
use ustr::Ustr;

use minion_ast::Model as MinionModel;
use minion_sys::ast as minion_ast;
use minion_sys::error::MinionError;
use minion_sys::{RunOptions, ValueOrder, run_minion_with_options};

use crate::Model as ConjureModel;
use crate::ast::{self as conjure_ast, Name};
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
use crate::solver::SolverError::{OpNotImplemented, Runtime, RuntimeNotImplemented};
use crate::solver::model_modifier::NotModifiable;
use crate::solver::private;

use super::parse_model::model_to_minion;

/// A [SolverAdaptor] for interacting with Minion.
///
/// This adaptor uses the `minion_sys` crate to talk to Minion over FFI.
pub struct Minion {
    __non_constructable: private::Internal,
    model: Option<MinionModel>,
    value_order: Option<MinionValueOrder>,
}

/// Value-order override for Minion search.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum MinionValueOrder {
    Ascend,
    Descend,
    Random,
}

impl From<MinionValueOrder> for ValueOrder {
    fn from(value: MinionValueOrder) -> Self {
        match value {
            MinionValueOrder::Ascend => ValueOrder::Ascend,
            MinionValueOrder::Descend => ValueOrder::Descend,
            MinionValueOrder::Random => ValueOrder::Random,
        }
    }
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
            value_order: None,
        }
    }

    /// Creates a Minion adaptor with an optional value-order override.
    pub fn with_value_order(value_order: Option<MinionValueOrder>) -> Minion {
        Minion {
            __non_constructable: private::Internal,
            model: None,
            value_order,
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

        let solver_ctx = run_minion_with_options(
            self.model.clone().expect("STATE MACHINE ERR"),
            Box::new(|solutions| {
                any_solutions = true;
                let conjure_solutions = translate_solution(solutions);
                let continue_search = callback(conjure_solutions);
                if !continue_search {
                    user_terminated = true;
                }
                continue_search
            }),
            RunOptions {
                value_order: self.value_order.map(Into::into),
            },
        )
        .map_err(|err| match err {
            MinionError::RuntimeError(x) => Runtime(format!("{x:#?}")),
            MinionError::Other(x) => Runtime(format!("{x:#?}")),
            MinionError::NotImplemented(x) => RuntimeNotImplemented(x),
            x => Runtime(format!("unknown minion_sys error: {x:#?}")),
        })?;

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
