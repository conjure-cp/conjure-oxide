use regex::Regex;
use std::collections::HashMap;
use std::sync::{LazyLock, Mutex, OnceLock};
use ustr::Ustr;

use minion_ast::Model as MinionModel;
use minion_sys::ast as minion_ast;
use minion_sys::error::MinionError;
use minion_sys::{get_from_table, run_minion};

use crate::Model as ConjureModel;
use crate::ast::{self as conjure_ast, Name};
use crate::solver::SolverCallback;
use crate::solver::SolverFamily;
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
}

static MINION_LOCK: Mutex<()> = Mutex::new(());
static USER_CALLBACK: OnceLock<Mutex<SolverCallback>> = OnceLock::new();
static ANY_SOLUTIONS: Mutex<bool> = Mutex::new(false);
static USER_TERMINATED: Mutex<bool> = Mutex::new(false);

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

#[allow(clippy::unwrap_used)]
fn minion_sys_callback(solutions: HashMap<minion_ast::VarName, minion_ast::Constant>) -> bool {
    *(ANY_SOLUTIONS.lock().unwrap()) = true;
    let callback = USER_CALLBACK
        .get_or_init(|| Mutex::new(Box::new(|x| true)))
        .lock()
        .unwrap();

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

    let continue_search = (**callback)(conjure_solutions);
    if !continue_search {
        *(USER_TERMINATED.lock().unwrap()) = true;
    }

    continue_search
}

impl private::Sealed for Minion {}

impl Minion {
    pub fn new() -> Minion {
        Minion {
            __non_constructable: private::Internal,
            model: None,
        }
    }
}

impl Default for Minion {
    fn default() -> Self {
        Minion::new()
    }
}

impl SolverAdaptor for Minion {
    #[allow(clippy::unwrap_used)]
    fn solve(
        &mut self,
        callback: SolverCallback,
        _: private::Internal,
    ) -> Result<SolveSuccess, SolverError> {
        // our minion callback is global state, so single threading the adaptor as a whole is
        // probably a good move...
        #[allow(clippy::unwrap_used)]
        let mut minion_lock = MINION_LOCK.lock().unwrap();

        #[allow(clippy::unwrap_used)]
        let mut user_callback = USER_CALLBACK
            .get_or_init(|| Mutex::new(Box::new(|x| true)))
            .lock()
            .unwrap();
        *user_callback = callback;
        drop(user_callback); // release mutex. REQUIRED so that run_minion can use the
        // user callback and not deadlock.

        run_minion(
            self.model.clone().expect("STATE MACHINE ERR"),
            minion_sys_callback,
        )
        .map_err(|err| match err {
            MinionError::RuntimeError(x) => Runtime(format!("{x:#?}")),
            MinionError::Other(x) => Runtime(format!("{x:#?}")),
            MinionError::NotImplemented(x) => RuntimeNotImplemented(x),
            x => Runtime(format!("unknown minion_sys error: {x:#?}")),
        })?;

        let mut status = Complete(HasSolutions);
        if *(USER_TERMINATED.lock()).unwrap() {
            status = Incomplete(UserTerminated);
        } else if *(ANY_SOLUTIONS.lock()).unwrap() {
            status = Complete(NoSolutions);
        }
        Ok(SolveSuccess {
            stats: get_solver_stats(),
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

    fn get_name(&self) -> Option<String> {
        Some("Minion".to_owned())
    }

    fn write_solver_input_file(
        &self,
        writer: &mut impl std::io::Write,
    ) -> Result<(), std::io::Error> {
        let model = self.model.as_ref().expect("Minion solver adaptor should have a model as write_solver_input_file should only be called in the LoadedModel state.");
        minion_sys::print::write_minion_file(writer, model)
    }
}

#[allow(clippy::unwrap_used)]
fn get_solver_stats() -> SolverStats {
    SolverStats {
        nodes: get_from_table("Nodes".into()).map(|x| x.parse::<u64>().unwrap()),
        ..Default::default()
    }
}
