//! Solver interface to minion_rs.

use crate::ast::{
    DecisionVariable, Domain as ConjureDomain, Expression as ConjureExpression,
    Model as ConjureModel, Name as ConjureName, Range as ConjureRange,
};
use minion_rs::ast::{
    Constraint as MinionConstraint, Model as MinionModel, Var as MinionVar,
    VarDomain as MinionDomain,
};

impl TryFrom<ConjureModel> for MinionModel {
    // TODO (nd60): set this to equal ConjureError once it is merged.
    type Error = String;

    fn try_from(conjure_model: ConjureModel) -> Result<Self, Self::Error> {
        let mut minion_model = MinionModel::new();

        // We assume (for now) that the conjure model is fully valid
        // i.e. type checked and the variables referenced all exist.
        parse_vars(&conjure_model, &mut minion_model)?;
        parse_exprs(&conjure_model, &mut minion_model)?;

        Ok(minion_model)
    }
}

fn parse_vars(conjure_model: &ConjureModel, minion_model: &mut MinionModel) -> Result<(), String> {
    // TODO (nd60): remove unused vars?
    // TODO (nd60): ensure all vars references are used.

    for (name, variable) in conjure_model.variables.iter() {
        parse_var(name, variable, minion_model)?;
    }
    Ok(())
}

fn parse_var(
    name: &ConjureName,
    variable: &DecisionVariable,
    minion_model: &mut MinionModel,
) -> Result<(), String> {
    let str_name = name_to_string(name.to_owned());

    let ranges = match &variable.domain {
        ConjureDomain::IntDomain(range) => Ok(range),
        x => Err(format!("Not supported: {:?}", x)),
    }?;

    // TODO (nd60): Currently, Minion only supports the use of one range in the domain.
    // If there are multiple ranges, SparseBound should be used here instead.
    // See: https://github.com/conjure-cp/conjure-oxide/issues/84

    if ranges.len() != 1 {
        return Err(format!(
            "Variable {:?} has {:?} ranges. Multiple ranges / SparseBound is not yet supported.",
            str_name,
            ranges.len()
        ));
    }

    let range = ranges
        .first()
        .ok_or(format!("Variable {:?} has no range", str_name))?;

    let (low, high) = match range {
        ConjureRange::Bounded(x, y) => Ok((x.to_owned(), y.to_owned())),
        ConjureRange::Single(x) => Ok((x.to_owned(), x.to_owned())),
        a => Err(format!("Not implemented {:?}", a)),
    }?;

    minion_model
        .named_variables
        .add_var(str_name.to_owned(), MinionDomain::Bound(low, high))
        .ok_or(format!("Variable {:?} is defined twice", str_name))?;

    Ok(())
}

fn parse_exprs(conjure_model: &ConjureModel, minion_model: &mut MinionModel) -> Result<(), String> {
    for expr in conjure_model.constraints.iter() {
        parse_expr(expr.to_owned(), minion_model)?;
    }
    Ok(())
}

fn parse_expr(expr: ConjureExpression, minion_model: &mut MinionModel) -> Result<(), String> {
    match expr {
        ConjureExpression::SumLeq(lhs, rhs) => parse_sumleq(lhs, *rhs, minion_model),
        ConjureExpression::SumGeq(lhs, rhs) => parse_sumgeq(lhs, *rhs, minion_model),
        x => Err(format!("Not supported: {:?}", x)),
    }
}

fn parse_sumleq(
    sum_vars: Vec<ConjureExpression>,
    rhs: ConjureExpression,
    minion_model: &mut MinionModel,
) -> Result<(), String> {
    let minion_vars = must_be_vars(sum_vars)?;
    let minion_rhs = must_be_var(rhs)?;
    minion_model
        .constraints
        .push(MinionConstraint::SumLeq(minion_vars, minion_rhs));

    Ok(())
}

fn parse_sumgeq(
    sum_vars: Vec<ConjureExpression>,
    rhs: ConjureExpression,
    minion_model: &mut MinionModel,
) -> Result<(), String> {
    let minion_vars = must_be_vars(sum_vars)?;
    let minion_rhs = must_be_var(rhs)?;
    minion_model
        .constraints
        .push(MinionConstraint::SumGeq(minion_vars, minion_rhs));

    Ok(())
}

fn must_be_vars(exprs: Vec<ConjureExpression>) -> Result<Vec<MinionVar>, String> {
    let mut minion_vars: Vec<MinionVar> = vec![];
    for expr in exprs {
        let minion_var = must_be_var(expr)?;
        minion_vars.push(minion_var);
    }
    Ok(minion_vars)
}

fn must_be_var(e: ConjureExpression) -> Result<MinionVar, String> {
    // a minion var is either a reference or a "var as const"
    match must_be_ref(e.clone()) {
        Ok(name) => Ok(MinionVar::NameRef(name)),
        Err(_) => match must_be_const(e) {
            Ok(n) => Ok(MinionVar::ConstantAsVar(n)),
            Err(x) => Err(x),
        },
    }
}

fn must_be_ref(e: ConjureExpression) -> Result<String, String> {
    let name = match e {
        ConjureExpression::Reference(n) => Ok(n),
        _ => Err(""),
    }?;

    // always use names as strings in Minon.
    let str_name = name_to_string(name);
    Ok(str_name)
}

fn must_be_const(e: ConjureExpression) -> Result<i32, String> {
    match e {
        ConjureExpression::ConstantInt(n) => Ok(n),
        _ => Err("".to_owned()),
    }
}

fn name_to_string(name: ConjureName) -> String {
    match name {
        ConjureName::UserName(x) => x,
        ConjureName::MachineName(x) => x.to_string(),
    }
}
