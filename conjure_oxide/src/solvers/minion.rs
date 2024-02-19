//! Solver interface to minion_rs.

use super::{FromConjureModel, SolverError};
use crate::Solver;

use crate::ast::{
    Constant as ConjureConstant, DecisionVariable, Domain as ConjureDomain,
    Expression as ConjureExpression, Model as ConjureModel, Name as ConjureName,
    Range as ConjureRange,
};

pub use minion_rs::ast::Model as MinionModel;
use minion_rs::ast::{
    Constant as MinionConstant, Constraint as MinionConstraint, Var as MinionVar,
    VarDomain as MinionDomain,
};

const SOLVER: Solver = Solver::Minion;

impl FromConjureModel for MinionModel {
    fn from_conjure(conjure_model: ConjureModel) -> Result<Self, SolverError> {
        let mut minion_model = MinionModel::new();

        // We assume (for now) that the conjure model is fully valid
        // i.e. type checked and the variables referenced all exist.
        parse_vars(&conjure_model, &mut minion_model)?;
        parse_exprs(&conjure_model, &mut minion_model)?;

        Ok(minion_model)
    }
}

fn parse_vars(
    conjure_model: &ConjureModel,
    minion_model: &mut MinionModel,
) -> Result<(), SolverError> {
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
) -> Result<(), SolverError> {
    let str_name = name_to_string(name.to_owned());

    let ranges = match &variable.domain {
        ConjureDomain::IntDomain(range) => Ok(range),
        x => Err(SolverError::NotSupported(SOLVER, format!("{:?}", x))),
    }?;

    // TODO (nd60): Currently, Minion only supports the use of one range in the domain.
    // If there are multiple ranges, SparseBound should be used here instead.
    // See: https://github.com/conjure-cp/conjure-oxide/issues/84

    if ranges.len() != 1 {
        return Err(SolverError::NotSupported(
            SOLVER,
            format!(
            "variable {:?} has {:?} ranges. Multiple ranges / SparseBound is not yet supported.",
            str_name,
            ranges.len()
        ),
        ));
    }

    let range = ranges.first().ok_or(SolverError::InvalidInstance(
        SOLVER,
        format!("variable {:?} has no range", str_name),
    ))?;

    let (low, high) = match range {
        ConjureRange::Bounded(x, y) => Ok((x.to_owned(), y.to_owned())),
        ConjureRange::Single(x) => Ok((x.to_owned(), x.to_owned())),
        #[allow(unreachable_patterns)]
        x => Err(SolverError::NotSupported(SOLVER, format!("{:?}", x))),
    }?;

    minion_model
        .named_variables
        .add_var(str_name.to_owned(), MinionDomain::Bound(low, high))
        .ok_or(SolverError::InvalidInstance(
            SOLVER,
            format!("variable {:?} is defined twice", str_name),
        ))?;

    Ok(())
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

fn parse_expr(expr: ConjureExpression, minion_model: &mut MinionModel) -> Result<(), SolverError> {
    match expr {
        ConjureExpression::SumLeq(lhs, rhs) => parse_sumleq(lhs, *rhs, minion_model),
        ConjureExpression::SumGeq(lhs, rhs) => parse_sumgeq(lhs, *rhs, minion_model),
        ConjureExpression::Ineq(a, b, c) => parse_ineq(*a, *b, *c, minion_model),
        x => Err(SolverError::NotSupported(SOLVER, format!("{:?}", x))),
    }
}

// fn parse_and(
//     expressions: Vec<Expression>,
//     minion_model: &mut MinionModel,
// ) -> Result<(), SolverError> {
//     // ToDo - Nik said that he will do this
// }

fn parse_sumleq(
    sum_vars: Vec<ConjureExpression>,
    rhs: ConjureExpression,
    minion_model: &mut MinionModel,
) -> Result<(), SolverError> {
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
) -> Result<(), SolverError> {
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
) -> Result<(), SolverError> {
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

fn must_be_vars(exprs: Vec<ConjureExpression>) -> Result<Vec<MinionVar>, SolverError> {
    let mut minion_vars: Vec<MinionVar> = vec![];
    for expr in exprs {
        let minion_var = must_be_var(expr)?;
        minion_vars.push(minion_var);
    }
    Ok(minion_vars)
}

fn must_be_var(e: ConjureExpression) -> Result<MinionVar, SolverError> {
    // a minion var is either a reference or a "var as const"
    match must_be_ref(e.clone()) {
        Ok(name) => Ok(MinionVar::NameRef(name)),
        Err(_) => match must_be_const(e) {
            Ok(n) => Ok(MinionVar::ConstantAsVar(n)),
            Err(x) => Err(x),
        },
    }
}

fn must_be_ref(e: ConjureExpression) -> Result<String, SolverError> {
    let name = match e {
        ConjureExpression::Reference(n) => Ok(n),
        x => Err(SolverError::InvalidInstance(
            SOLVER,
            format!("expected a reference, but got `{0:?}`", x),
        )),
    }?;

    let str_name = name_to_string(name);
    Ok(str_name)
}

fn must_be_const(e: ConjureExpression) -> Result<i32, SolverError> {
    match e {
        ConjureExpression::Constant(_, ConjureConstant::Int(n)) => Ok(n),
        x => Err(SolverError::InvalidInstance(
            SOLVER,
            format!("expected a constant, but got `{0:?}`", x),
        )),
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
    use anyhow::anyhow;
    use conjure_core::ast::Expression;
    use std::collections::HashMap;

    use minion_rs::ast::VarName;

    use super::*;

    #[test]
    fn flat_xyz_model() -> Result<(), anyhow::Error> {
        // TODO: convert to use public interfaces when these exist.
        let mut model = ConjureModel {
            variables: HashMap::new(),
            constraints: Expression::And(Vec::new()),
        };

        add_int_with_range(&mut model, "x", 1, 3)?;
        add_int_with_range(&mut model, "y", 2, 4)?;
        add_int_with_range(&mut model, "z", 1, 5)?;

        let x = ConjureExpression::Reference(ConjureName::UserName("x".to_owned()));
        let y = ConjureExpression::Reference(ConjureName::UserName("y".to_owned()));
        let z = ConjureExpression::Reference(ConjureName::UserName("z".to_owned()));
        let four = ConjureExpression::Constant(Metadata::new(), ConjureConstant::Int(4));

        let geq = ConjureExpression::SumGeq(
            vec![x.to_owned(), y.to_owned(), z.to_owned()],
            Box::from(four.to_owned()),
        );
        let leq = ConjureExpression::SumLeq(
            vec![x.to_owned(), y.to_owned(), z.to_owned()],
            Box::from(four.to_owned()),
        );
        let ineq = ConjureExpression::Ineq(Box::from(x), Box::from(y), Box::from(four));

        model.add_constraints(vec![geq, leq, ineq]);

        let minion_model = MinionModel::from_conjure(model)?;
        Ok(minion_rs::run_minion(minion_model, xyz_callback)?)
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
    ) -> Result<(), SolverError> {
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
        if res.is_some() {
            return Err(SolverError::Other(anyhow!(
                "Tried to add variable {:?} to the symbol table, but it was already present",
                name
            )));
        }

        Ok(())
    }
}
