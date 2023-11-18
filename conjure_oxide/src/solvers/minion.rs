//! Solver interface to minion_rs.

use crate::ast::{
    DecisionVariable, Domain as ConjureDomain, Expression as ConjureExpression,
    Model as ConjureModel, Name as ConjureName, Range as ConjureRange,
};
use minion_rs::ast::{
    Constant as MinionConstant, Constraint as MinionConstraint, Model as MinionModel,
    Var as MinionVar, VarDomain as MinionDomain,
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
        ConjureExpression::Ineq(a, b, c) => parse_ineq(*a, *b, *c, minion_model),
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

fn parse_ineq(
    a: ConjureExpression,
    b: ConjureExpression,
    c: ConjureExpression,
    minion_model: &mut MinionModel,
) -> Result<(), String> {
    let a_minion = must_be_var(a)?;
    let b_minion = must_be_var(b)?;
    let c_value = must_be_const(c)?;
    minion_model.constraints.push(MinionConstraint::Ineq(
        a_minion,
        b_minion,
        MinionConstant::Integer(c_value),
    ));

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

#[cfg(test)]
mod tests {
    use std::collections::HashMap;

    use minion_rs::ast::VarName;

    use super::*;

    #[test]
    fn flat_xyz_model() -> Result<(), String> {
        // TODO: convert to use public interfaces when these exist.
        let mut model = ConjureModel {
            variables: HashMap::new(),
            constraints: Vec::new(),
        };

        add_int_with_range(&mut model, "x", 1, 3)?;
        add_int_with_range(&mut model, "y", 2, 4)?;
        add_int_with_range(&mut model, "z", 1, 5)?;

        let x = ConjureExpression::Reference(ConjureName::UserName("x".to_owned()));
        let y = ConjureExpression::Reference(ConjureName::UserName("y".to_owned()));
        let z = ConjureExpression::Reference(ConjureName::UserName("z".to_owned()));
        let four = ConjureExpression::ConstantInt(4);

        let geq = ConjureExpression::SumGeq(
            vec![x.to_owned(), y.to_owned(), z.to_owned()],
            Box::from(four.to_owned()),
        );
        let leq = ConjureExpression::SumLeq(
            vec![x.to_owned(), y.to_owned(), z.to_owned()],
            Box::from(four.to_owned()),
        );
        let ineq = ConjureExpression::Ineq(Box::from(x), Box::from(y), Box::from(four));

        model.constraints.push(geq);
        model.constraints.push(leq);
        model.constraints.push(ineq);

        let minion_model = MinionModel::try_from(model)?;
        minion_rs::run_minion(minion_model, xyz_callback).map_err(|x| x.to_string())
    }

    #[allow(clippy::unwrap_used)]
    fn xyz_callback(solutions: HashMap<VarName, MinionConstant>) -> bool {
        let x = match solutions.get("x").unwrap() {
            MinionConstant::Integer(n) => n,
            _ => panic!("x should be a integer"),
        };

        let y = match solutions.get("y").unwrap() {
            MinionConstant::Integer(n) => n,
            _ => panic!("y should be a integer"),
        };

        let z = match solutions.get("z").unwrap() {
            MinionConstant::Integer(n) => n,
            _ => panic!("z should be a integer"),
        };

        assert_eq!(*x, 1);
        assert_eq!(*y, 2);
        assert_eq!(*z, 1);

        false
    }

    fn add_int_with_range(
        model: &mut ConjureModel,
        name: &str,
        domain_low: i32,
        domain_high: i32,
    ) -> Result<(), String> {
        // TODO: convert to use public interfaces when these exist.
        let res = model.variables.insert(
            ConjureName::UserName(name.to_owned()),
            DecisionVariable {
                domain: ConjureDomain::IntDomain(vec![ConjureRange::Bounded(
                    domain_low,
                    domain_high,
                )]),
            },
        );

        match res {
            // variable was not already present
            None => Ok(()),
            Some(_) => Err(format!("Variable {:?} was already present", name)),
        }
    }
}
