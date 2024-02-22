//! Solver interface to minion_rs.

#![allow(unreachable_patterns)]

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
    VarDomain as MinionDomain, VarName,
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
    var: &DecisionVariable,
    minion_model: &mut MinionModel,
) -> Result<(), SolverError> {
    match &var.domain {
        ConjureDomain::IntDomain(ranges) => _parse_intdomain_var(name, ranges, minion_model),
        ConjureDomain::BoolDomain => _parse_booldomain_var(name, minion_model),
        x => Err(SolverError::NotSupported(SOLVER, format!("{:?}", x))),
    }
}

fn _parse_intdomain_var(
    name: &ConjureName,
    ranges: &Vec<ConjureRange<i32>>,
    minion_model: &mut MinionModel,
) -> Result<(), SolverError> {
    let str_name = _name_to_string(name.to_owned());

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

    _try_add_var(
        str_name.to_owned(),
        MinionDomain::Bound(low, high),
        minion_model,
    )
}

fn _parse_booldomain_var(
    name: &ConjureName,
    minion_model: &mut MinionModel,
) -> Result<(), SolverError> {
    let str_name = _name_to_string(name.to_owned());
    _try_add_var(str_name.to_owned(), MinionDomain::Bool, minion_model)
}

fn _try_add_var(
    name: VarName,
    domain: MinionDomain,
    minion_model: &mut MinionModel,
) -> Result<(), SolverError> {
    minion_model
        .named_variables
        .add_var(name.clone(), domain)
        .ok_or(SolverError::InvalidInstance(
            SOLVER,
            format!("variable {:?} is defined twice", name),
        ))
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
        ConjureExpression::SumLeq(_metadata, lhs, rhs) => {
            minion_model
                .constraints
                .push(MinionConstraint::SumLeq(read_vars(lhs)?, read_var(*rhs)?));
            Ok(())
        }
        ConjureExpression::SumGeq(_metadata, lhs, rhs) => {
            minion_model
                .constraints
                .push(MinionConstraint::SumGeq(read_vars(lhs)?, read_var(*rhs)?));
            Ok(())
        }
        ConjureExpression::Ineq(_metadata, a, b, c) => {
            minion_model.constraints.push(MinionConstraint::Ineq(
                read_var(*a)?,
                read_var(*b)?,
                MinionConstant::Integer(read_const(*c)?),
            ));
            Ok(())
        }
        ConjureExpression::Neq(_metadata, a, b) => {
            minion_model
                .constraints
                .push(MinionConstraint::AllDiff(vec![
                    read_var(*a)?,
                    read_var(*b)?,
                ]));
            Ok(())
        }
        ConjureExpression::DivEq(_metadata, a, b, c) => {
            minion_model.constraints.push(MinionConstraint::Div(
                (read_var(*a)?, read_var(*b)?),
                read_var(*c)?,
            ));
            Ok(())
        }
        ConjureExpression::AllDiff(_metadata, vars) => {
            minion_model
                .constraints
                .push(MinionConstraint::AllDiff(read_vars(vars)?));
            Ok(())
        }
        x => Err(SolverError::NotSupported(SOLVER, format!("{:?}", x))),
    }
}

fn read_vars(exprs: Vec<ConjureExpression>) -> Result<Vec<MinionVar>, SolverError> {
    let mut minion_vars: Vec<MinionVar> = vec![];
    for expr in exprs {
        let minion_var = read_var(expr)?;
        minion_vars.push(minion_var);
    }
    Ok(minion_vars)
}

fn read_var(e: ConjureExpression) -> Result<MinionVar, SolverError> {
    // a minion var is either a reference or a "var as const"
    match _read_ref(e.clone()) {
        Ok(name) => Ok(MinionVar::NameRef(name)),
        Err(_) => match read_const(e) {
            Ok(n) => Ok(MinionVar::ConstantAsVar(n)),
            Err(x) => Err(x),
        },
    }
}

fn _read_ref(e: ConjureExpression) -> Result<String, SolverError> {
    let name = match e {
        ConjureExpression::Reference(_metdata, n) => Ok(n),
        x => Err(SolverError::InvalidInstance(
            SOLVER,
            format!("expected a reference, but got `{0:?}`", x),
        )),
    }?;

    let str_name = _name_to_string(name);
    Ok(str_name)
}

fn read_const(e: ConjureExpression) -> Result<i32, SolverError> {
    match e {
        ConjureExpression::Constant(_, ConjureConstant::Int(n)) => Ok(n),
        x => Err(SolverError::InvalidInstance(
            SOLVER,
            format!("expected a constant, but got `{0:?}`", x),
        )),
    }
}

fn _name_to_string(name: ConjureName) -> String {
    match name {
        ConjureName::UserName(x) => x,
        ConjureName::MachineName(x) => x.to_string(),
    }
}

#[cfg(test)]
mod tests {
    use anyhow::anyhow;
    use conjure_core::ast::Expression;
    use conjure_core::metadata::Metadata;
    use std::collections::HashMap;

    use minion_rs::ast::VarName;

    use super::*;

    #[test]
    fn flat_xyz_model() -> Result<(), anyhow::Error> {
        // TODO: convert to use public interfaces when these exist.
        let mut model = ConjureModel {
            variables: HashMap::new(),
            constraints: Expression::And(Metadata::new(), Vec::new()),
        };

        add_int_with_range(&mut model, "x", 1, 3)?;
        add_int_with_range(&mut model, "y", 2, 4)?;
        add_int_with_range(&mut model, "z", 1, 5)?;

        let x =
            ConjureExpression::Reference(Metadata::new(), ConjureName::UserName("x".to_owned()));
        let y =
            ConjureExpression::Reference(Metadata::new(), ConjureName::UserName("y".to_owned()));
        let z =
            ConjureExpression::Reference(Metadata::new(), ConjureName::UserName("z".to_owned()));
        let four = ConjureExpression::Constant(Metadata::new(), ConjureConstant::Int(4));

        let geq = ConjureExpression::SumGeq(
            Metadata::new(),
            vec![x.to_owned(), y.to_owned(), z.to_owned()],
            Box::from(four.to_owned()),
        );
        let leq = ConjureExpression::SumLeq(
            Metadata::new(),
            vec![x.to_owned(), y.to_owned(), z.to_owned()],
            Box::from(four.to_owned()),
        );
        let ineq =
            ConjureExpression::Ineq(Metadata::new(), Box::from(x), Box::from(y), Box::from(four));

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
