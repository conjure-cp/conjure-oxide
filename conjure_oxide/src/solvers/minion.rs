//! Solver interface to minion_rs.

use crate::ast::{Expression as ConjureExpression, Model as ConjureModel, Name as ConjureName};
use minion_rs::ast::{Constraint as MinionConstraint, Model as MinionModel, Var as MinionVar};

impl TryFrom<ConjureModel> for MinionModel {
    // TODO: set this to equal ConjureError once it is merged.
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

#[allow(unused_variables)]
fn parse_vars(conjure_model: &ConjureModel, minion_model: &MinionModel) -> Result<(), String> {
    todo!();
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
    match name {
        ConjureName::UserName(x) => Ok(x),
        ConjureName::MachineName(x) => Ok(x.to_string()),
    }
}

fn must_be_const(e: ConjureExpression) -> Result<i32, String> {
    match e {
        ConjureExpression::ConstantInt(n) => Ok(n),
        _ => Err("".to_owned()),
    }
}
