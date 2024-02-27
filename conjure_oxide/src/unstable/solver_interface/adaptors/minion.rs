use crate::unstable::solver_interface::states;

use super::super::model_modifier::NotModifiable;
use super::super::private;
use super::super::SolveSuccess;
use super::super::SolverAdaptor;
use super::super::SolverError;
use super::super::SolverError::*;

use crate::ast as conjureast;
use minion_rs::ast as minionast;

/// A [SolverAdaptor] for interacting with Minion.
///
/// This adaptor uses the `minion_rs` crate to talk to Minion over FFI.
pub struct Minion;

impl private::Sealed for Minion {}
impl SolverAdaptor for Minion {
    type Model = minionast::Model;
    type Solution = minionast::Constant;
    type Modifier = NotModifiable;

    fn solve(
        &mut self,
        model: Self::Model,
        callback: fn(std::collections::HashMap<String, String>) -> bool,
        _: private::Internal,
    ) -> Result<SolveSuccess, SolverError> {
        Err(OpNotImplemented("solve".into()))
    }

    fn solve_mut(
        &mut self,
        model: Self::Model,
        callback: fn(std::collections::HashMap<String, String>, Self::Modifier) -> bool,
        _: private::Internal,
    ) -> Result<SolveSuccess, SolverError> {
        Err(OpNotImplemented("solve_mut".into()))
    }

    fn load_model(
        &mut self,
        model: conjure_core::ast::Model,
        _: private::Internal,
    ) -> Result<Self::Model, SolverError> {
        let mut minion_model = minionast::Model::new();
        parse_vars(&model, &mut minion_model)?;
        parse_exprs(&model, &mut minion_model)?;
        Ok(minion_model)
    }
}

fn parse_vars(
    conjure_model: &conjureast::Model,
    minion_model: &mut minionast::Model,
) -> Result<(), SolverError> {
    // TODO (nd60): remove unused vars?
    // TODO (nd60): ensure all vars references are used.

    for (name, variable) in conjure_model.variables.iter() {
        parse_var(name, variable, minion_model)?;
    }
    Ok(())
}

fn parse_var(
    name: &conjureast::Name,
    var: &conjureast::DecisionVariable,
    minion_model: &mut minionast::Model,
) -> Result<(), SolverError> {
    match &var.domain {
        conjureast::Domain::IntDomain(ranges) => _parse_intdomain_var(name, ranges, minion_model),
        conjureast::Domain::BoolDomain => _parse_booldomain_var(name, minion_model),
        x => Err(ModelFeatureNotSupported(format!("{:?}", x))),
    }
}

fn _parse_intdomain_var(
    name: &conjureast::Name,
    ranges: &Vec<conjureast::Range<i32>>,
    minion_model: &mut minionast::Model,
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
        conjureast::Range::Bounded(x, y) => Ok((x.to_owned(), y.to_owned())),
        conjureast::Range::Single(x) => Ok((x.to_owned(), x.to_owned())),
        #[allow(unreachable_patterns)]
        x => Err(ModelFeatureNotSupported(format!("{:?}", x))),
    }?;

    _try_add_var(
        str_name.to_owned(),
        minionast::VarDomain::Bound(low, high),
        minion_model,
    )
}

fn _parse_booldomain_var(
    name: &conjureast::Name,
    minion_model: &mut minionast::Model,
) -> Result<(), SolverError> {
    let str_name = _name_to_string(name.to_owned());
    _try_add_var(
        str_name.to_owned(),
        minionast::VarDomain::Bool,
        minion_model,
    )
}

fn _try_add_var(
    name: minionast::VarName,
    domain: minionast::VarDomain,
    minion_model: &mut minionast::Model,
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
    conjure_model: &conjureast::Model,
    minion_model: &mut minionast::Model,
) -> Result<(), SolverError> {
    for expr in conjure_model.get_constraints_vec().iter() {
        parse_expr(expr.to_owned(), minion_model)?;
    }
    Ok(())
}

fn parse_expr(
    expr: conjureast::Expression,
    minion_model: &mut minionast::Model,
) -> Result<(), SolverError> {
    match expr {
        conjureast::Expression::SumLeq(_metadata, lhs, rhs) => {
            minion_model.constraints.push(minionast::Constraint::SumLeq(
                read_vars(lhs)?,
                read_var(*rhs)?,
            ));
            Ok(())
        }
        conjureast::Expression::SumGeq(_metadata, lhs, rhs) => {
            minion_model.constraints.push(minionast::Constraint::SumGeq(
                read_vars(lhs)?,
                read_var(*rhs)?,
            ));
            Ok(())
        }
        conjureast::Expression::Ineq(_metadata, a, b, c) => {
            minion_model.constraints.push(minionast::Constraint::Ineq(
                read_var(*a)?,
                read_var(*b)?,
                minionast::Constant::Integer(read_const(*c)?),
            ));
            Ok(())
        }
        conjureast::Expression::Neq(_metadata, a, b) => {
            minion_model
                .constraints
                .push(minionast::Constraint::WatchNeq(
                    read_var(*a)?,
                    read_var(*b)?,
                ));
            Ok(())
        }
        x => Err(ModelFeatureNotSupported(format!("{:?}", x))),
    }
}

fn read_vars(exprs: Vec<conjureast::Expression>) -> Result<Vec<minionast::Var>, SolverError> {
    let mut minion_vars: Vec<minionast::Var> = vec![];
    for expr in exprs {
        let minion_var = read_var(expr)?;
        minion_vars.push(minion_var);
    }
    Ok(minion_vars)
}

fn read_var(e: conjureast::Expression) -> Result<minionast::Var, SolverError> {
    // a minion var is either a reference or a "var as const"
    match _read_ref(e.clone()) {
        Ok(name) => Ok(minionast::Var::NameRef(name)),
        Err(_) => match read_const(e) {
            Ok(n) => Ok(minionast::Var::ConstantAsVar(n)),
            Err(x) => Err(x),
        },
    }
}

fn _read_ref(e: conjureast::Expression) -> Result<String, SolverError> {
    let name = match e {
        conjureast::Expression::Reference(_metdata, n) => Ok(n),
        x => Err(ModelInvalid(format!(
            "expected a reference, but got `{0:?}`",
            x
        ))),
    }?;

    let str_name = _name_to_string(name);
    Ok(str_name)
}

fn read_const(e: conjureast::Expression) -> Result<i32, SolverError> {
    match e {
        conjureast::Expression::Constant(_, conjureast::Constant::Int(n)) => Ok(n),
        x => Err(ModelInvalid(format!(
            "expected a constant, but got `{0:?}`",
            x
        ))),
    }
}

fn _name_to_string(name: conjureast::Name) -> String {
    match name {
        conjureast::Name::UserName(x) => x,
        conjureast::Name::MachineName(x) => x.to_string(),
    }
}
