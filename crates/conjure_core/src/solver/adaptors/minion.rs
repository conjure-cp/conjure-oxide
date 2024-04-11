use std::collections::HashMap;
use std::sync::{Mutex, OnceLock};

use regex::Regex;

use minion_ast::Model as MinionModel;
use minion_rs::ast as minion_ast;
use minion_rs::error::MinionError;
use minion_rs::{get_from_table, run_minion};

use crate::ast as conjure_ast;
use crate::solver::SolverCallback;
use crate::solver::SolverFamily;
use crate::solver::SolverMutCallback;
use crate::stats::SolverStats;
use crate::Model as ConjureModel;

use super::super::model_modifier::NotModifiable;
use super::super::private;
use super::super::SearchComplete::*;
use super::super::SearchIncomplete::*;
use super::super::SearchStatus::*;
use super::super::SolveSuccess;
use super::super::SolverAdaptor;
use super::super::SolverError;
use super::super::SolverError::*;

/// A [SolverAdaptor] for interacting with Minion.
///
/// This adaptor uses the `minion_rs` crate to talk to Minion over FFI.
pub struct Minion {
    __non_constructable: private::Internal,
    model: Option<MinionModel>,
}

static MINION_LOCK: Mutex<()> = Mutex::new(());
static USER_CALLBACK: OnceLock<Mutex<SolverCallback>> = OnceLock::new();
static ANY_SOLUTIONS: Mutex<bool> = Mutex::new(false);
static USER_TERMINATED: Mutex<bool> = Mutex::new(false);

#[allow(clippy::unwrap_used)]
fn minion_rs_callback(solutions: HashMap<minion_ast::VarName, minion_ast::Constant>) -> bool {
    *(ANY_SOLUTIONS.lock().unwrap()) = true;
    let callback = USER_CALLBACK
        .get_or_init(|| Mutex::new(Box::new(|x| true)))
        .lock()
        .unwrap();

    let mut conjure_solutions: HashMap<conjure_ast::Name, conjure_ast::Constant> = HashMap::new();
    for (minion_name, minion_const) in solutions.into_iter() {
        let conjure_const = match minion_const {
            minion_ast::Constant::Bool(x) => conjure_ast::Constant::Bool(x),
            minion_ast::Constant::Integer(x) => conjure_ast::Constant::Int(x),
            _ => todo!(),
        };

        let machine_name_re = Regex::new(r"__conjure_machine_name_([0-9]+)").unwrap();
        let conjure_name = if let Some(caps) = machine_name_re.captures(&minion_name) {
            conjure_ast::Name::MachineName(caps[1].parse::<i32>().unwrap())
        } else {
            conjure_ast::Name::UserName(minion_name)
        };

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
            minion_rs_callback,
        )
        .map_err(|err| match err {
            MinionError::RuntimeError(x) => Runtime(format!("{:#?}", x)),
            MinionError::Other(x) => Runtime(format!("{:#?}", x)),
            MinionError::NotImplemented(x) => RuntimeNotImplemented(x),
            x => Runtime(format!("unknown minion_rs error: {:#?}", x)),
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
        let mut minion_model = MinionModel::new();
        parse_vars(&model, &mut minion_model)?;
        parse_exprs(&model, &mut minion_model)?;
        self.model = Some(minion_model);
        Ok(())
    }

    fn get_family(&self) -> SolverFamily {
        SolverFamily::Minion
    }

    fn get_name(&self) -> Option<String> {
        Some("Minion".to_owned())
    }
}

fn parse_vars(
    conjure_model: &ConjureModel,
    minion_model: &mut MinionModel,
) -> Result<(), SolverError> {
    // TODO (niklasdewally): remove unused vars?
    // TODO (niklasdewally): ensure all vars references are used.

    for (name, variable) in conjure_model.variables.iter() {
        parse_var(name, variable, minion_model)?;
    }
    Ok(())
}

fn parse_var(
    name: &conjure_ast::Name,
    var: &conjure_ast::DecisionVariable,
    minion_model: &mut MinionModel,
) -> Result<(), SolverError> {
    match &var.domain {
        conjure_ast::Domain::IntDomain(ranges) => _parse_intdomain_var(name, ranges, minion_model),
        conjure_ast::Domain::BoolDomain => _parse_booldomain_var(name, minion_model),
        x => Err(ModelFeatureNotSupported(format!("{:?}", x))),
    }
}

fn _parse_intdomain_var(
    name: &conjure_ast::Name,
    ranges: &[conjure_ast::Range<i32>],
    minion_model: &mut MinionModel,
) -> Result<(), SolverError> {
    let str_name = _name_to_string(name.to_owned());

    if ranges.len() != 1 {
        return Err(ModelFeatureNotImplemented(format!(
            "variable {:?} has {:?} ranges. Multiple ranges / SparseBound is not yet supported.",
            str_name,
            ranges.len()
        )));
    }

    let range = ranges.first().ok_or(ModelInvalid(format!(
        "variable {:?} has no range",
        str_name
    )))?;

    let (low, high) = match range {
        conjure_ast::Range::Bounded(x, y) => Ok((x.to_owned(), y.to_owned())),
        conjure_ast::Range::Single(x) => Ok((x.to_owned(), x.to_owned())),
        #[allow(unreachable_patterns)]
        x => Err(ModelFeatureNotSupported(format!("{:?}", x))),
    }?;

    _try_add_var(
        str_name.to_owned(),
        minion_ast::VarDomain::Bound(low, high),
        minion_model,
    )
}

fn _parse_booldomain_var(
    name: &conjure_ast::Name,
    minion_model: &mut MinionModel,
) -> Result<(), SolverError> {
    let str_name = _name_to_string(name.to_owned());
    _try_add_var(
        str_name.to_owned(),
        minion_ast::VarDomain::Bool,
        minion_model,
    )
}

fn _try_add_var(
    name: minion_ast::VarName,
    domain: minion_ast::VarDomain,
    minion_model: &mut MinionModel,
) -> Result<(), SolverError> {
    minion_model
        .named_variables
        .add_var(name.clone(), domain)
        .ok_or(ModelInvalid(format!(
            "variable {:?} is defined twice",
            name
        )))
}

fn parse_exprs(
    conjure_model: &ConjureModel,
    minion_model: &mut MinionModel,
) -> Result<(), SolverError> {
    for expr in conjure_model.get_constraints_vec().iter() {
        parse_expr(expr.to_owned(), minion_model)?;
    }
    Ok(())
}

fn parse_expr(
    expr: conjure_ast::Expression,
    minion_model: &mut MinionModel,
) -> Result<(), SolverError> {
    minion_model.constraints.push(read_expr(expr)?);
    Ok(())
}

fn read_expr(expr: conjure_ast::Expression) -> Result<minion_ast::Constraint, SolverError> {
    match expr {
        conjure_ast::Expression::SumLeq(_metadata, lhs, rhs) => Ok(minion_ast::Constraint::SumLeq(
            read_vars(lhs)?,
            read_var(*rhs)?,
        )),
        conjure_ast::Expression::SumGeq(_metadata, lhs, rhs) => Ok(minion_ast::Constraint::SumGeq(
            read_vars(lhs)?,
            read_var(*rhs)?,
        )),
        conjure_ast::Expression::Ineq(_metadata, a, b, c) => Ok(minion_ast::Constraint::Ineq(
            read_var(*a)?,
            read_var(*b)?,
            minion_ast::Constant::Integer(read_const(*c)?),
        )),
        conjure_ast::Expression::Neq(_metadata, a, b) => {
            Ok(minion_ast::Constraint::DisEq(read_var(*a)?, read_var(*b)?))
        }
        conjure_ast::Expression::DivEq(_metadata, a, b, c) => Ok(
            minion_ast::Constraint::DivUndefZero((read_var(*a)?, read_var(*b)?), read_var(*c)?),
        ),
        conjure_ast::Expression::Or(_metadata, exprs) => Ok(minion_ast::Constraint::WatchedOr(
            exprs
                .iter()
                .map(|x| read_expr(x.to_owned()))
                .collect::<Result<Vec<minion_ast::Constraint>, SolverError>>()?,
        )),
        conjure_ast::Expression::Eq(_metadata, a, b) => {
            Ok(minion_ast::Constraint::Eq(read_var(*a)?, read_var(*b)?))
        }
        x => Err(ModelFeatureNotSupported(format!("{:?}", x))),
    }
}
fn read_vars(exprs: Vec<conjure_ast::Expression>) -> Result<Vec<minion_ast::Var>, SolverError> {
    let mut minion_vars: Vec<minion_ast::Var> = vec![];
    for expr in exprs {
        let minion_var = read_var(expr)?;
        minion_vars.push(minion_var);
    }
    Ok(minion_vars)
}

fn read_var(e: conjure_ast::Expression) -> Result<minion_ast::Var, SolverError> {
    // a minion var is either a reference or a "var as const"
    match _read_ref(e.clone()) {
        Ok(name) => Ok(minion_ast::Var::NameRef(name)),
        Err(_) => match read_const(e) {
            Ok(n) => Ok(minion_ast::Var::ConstantAsVar(n)),
            Err(x) => Err(x),
        },
    }
}

fn _read_ref(e: conjure_ast::Expression) -> Result<String, SolverError> {
    let name = match e {
        conjure_ast::Expression::Reference(_metadata, n) => Ok(n),
        x => Err(ModelInvalid(format!(
            "expected a reference, but got `{0:?}`",
            x
        ))),
    }?;

    let str_name = _name_to_string(name);
    Ok(str_name)
}

fn read_const(e: conjure_ast::Expression) -> Result<i32, SolverError> {
    match e {
        conjure_ast::Expression::Constant(_, conjure_ast::Constant::Int(n)) => Ok(n),
        x => Err(ModelInvalid(format!(
            "expected a constant, but got `{0:?}`",
            x
        ))),
    }
}

fn _name_to_string(name: conjure_ast::Name) -> String {
    match name {
        conjure_ast::Name::UserName(x) => x,
        conjure_ast::Name::MachineName(x) => format!("__conjure_machine_name_{}", x),
    }
}

#[allow(clippy::unwrap_used)]
fn get_solver_stats() -> SolverStats {
    SolverStats {
        nodes: get_from_table("Nodes".into()).map(|x| x.parse::<u64>().unwrap()),
        ..Default::default()
    }
}
