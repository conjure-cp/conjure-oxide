//! Parse / `load_model` step of running Minion.

use minion_ast::Model as MinionModel;
use minion_rs::ast as minion_ast;
use minion_rs::error::MinionError;
use minion_rs::{get_from_table, run_minion};

use crate::ast as conjure_ast;
use crate::solver::SolverError::*;
use crate::solver::SolverFamily;
use crate::solver::SolverMutCallback;
use crate::solver::{SolverCallback, SolverError};
use crate::stats::SolverStats;
use crate::Model as ConjureModel;

/// Converts a conjure-oxide model to a `minion_rs` model.
pub fn model_to_minion(model: ConjureModel) -> Result<MinionModel, SolverError> {
    let mut minion_model = MinionModel::new();
    load_symbol_table(&model, &mut minion_model)?;
    load_constraints(&model, &mut minion_model)?;
    Ok(minion_model)
}

/// Loads the symbol table into `minion_model`.
fn load_symbol_table(
    conjure_model: &ConjureModel,
    minion_model: &mut MinionModel,
) -> Result<(), SolverError> {
    for (name, variable) in conjure_model.variables.iter() {
        load_var(name, variable, minion_model)?;
    }
    Ok(())
}

/// Loads a single variable into `minion_model`
fn load_var(
    name: &conjure_ast::Name,
    var: &conjure_ast::DecisionVariable,
    minion_model: &mut MinionModel,
) -> Result<(), SolverError> {
    match &var.domain {
        conjure_ast::Domain::IntDomain(ranges) => load_intdomain_var(name, ranges, minion_model),
        conjure_ast::Domain::BoolDomain => load_booldomain_var(name, minion_model),
        x => Err(ModelFeatureNotSupported(format!("{:?}", x))),
    }
}

/// Loads a variable with domain IntDomain into `minion_model`
fn load_intdomain_var(
    name: &conjure_ast::Name,
    ranges: &[conjure_ast::Range<i32>],
    minion_model: &mut MinionModel,
) -> Result<(), SolverError> {
    let str_name = name_to_string(name.to_owned());

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

/// Loads a variable with domain BoolDomain into `minion_model`
fn load_booldomain_var(
    name: &conjure_ast::Name,
    minion_model: &mut MinionModel,
) -> Result<(), SolverError> {
    let str_name = name_to_string(name.to_owned());
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

fn name_to_string(name: conjure_ast::Name) -> String {
    match name {
        conjure_ast::Name::UserName(x) => x,
        conjure_ast::Name::MachineName(x) => format!("__conjure_machine_name_{}", x),
    }
}

/// Loads the constraints into `minion_model`.
fn load_constraints(
    conjure_model: &ConjureModel,
    minion_model: &mut MinionModel,
) -> Result<(), SolverError> {
    for expr in conjure_model.get_constraints_vec().iter() {
        // TODO: top level false / trues should not go to the solver to begin with
        // ... but changing this at this stage would require rewriting the tester
        use crate::metadata::Metadata;
        use conjure_ast::Atom;
        use conjure_ast::Expression as Expr;
        use conjure_ast::Literal::*;

        match expr {
            // top level false
            Expr::Atomic(_, Atom::Literal(Bool(false))) => {
                minion_model.constraints.push(minion_ast::Constraint::False);
            }
            // top level true
            Expr::Atomic(_, Atom::Literal(Bool(true))) => {
                minion_model.constraints.push(minion_ast::Constraint::True);
            }

            _ => {
                load_expr(expr.to_owned(), minion_model)?;
            }
        }
    }
    Ok(())
}

/// Adds `expr` to `minion_model`
fn load_expr(
    expr: conjure_ast::Expression,
    minion_model: &mut MinionModel,
) -> Result<(), SolverError> {
    minion_model.constraints.push(parse_expr(expr)?);
    Ok(())
}

/// Parses a Conjure Oxide expression into a `minion_rs` constraint.
fn parse_expr(expr: conjure_ast::Expression) -> Result<minion_ast::Constraint, SolverError> {
    match expr {
        conjure_ast::Expression::Atomic(_metadata, atom) => Ok(minion_ast::Constraint::WLiteral(
            parse_atom(atom)?,
            minion_ast::Constant::Integer(1),
        )),
        conjure_ast::Expression::FlatSumLeq(_metadata, lhs, rhs) => Ok(
            minion_ast::Constraint::SumLeq(parse_atoms(lhs)?, parse_atom(rhs)?),
        ),
        conjure_ast::Expression::FlatSumGeq(_metadata, lhs, rhs) => Ok(
            minion_ast::Constraint::SumGeq(parse_atoms(lhs)?, parse_atom(rhs)?),
        ),
        conjure_ast::Expression::FlatIneq(_metadata, a, b, c) => Ok(minion_ast::Constraint::Ineq(
            parse_atom(a)?,
            parse_atom(b)?,
            parse_literal(c)?,
        )),
        conjure_ast::Expression::Neq(_metadata, a, b) => Ok(minion_ast::Constraint::DisEq(
            parse_atomic_expr(*a)?,
            parse_atomic_expr(*b)?,
        )),
        conjure_ast::Expression::MinionDivEqUndefZero(_metadata, a, b, c) => Ok(
            minion_ast::Constraint::DivUndefZero((parse_atom(a)?, parse_atom(b)?), parse_atom(c)?),
        ),
        conjure_ast::Expression::MinionModuloEqUndefZero(_metadata, a, b, c) => {
            Ok(minion_ast::Constraint::ModuloUndefZero(
                (parse_atom(a)?, parse_atom(b)?),
                parse_atom(c)?,
            ))
        }
        conjure_ast::Expression::Or(_metadata, exprs) => Ok(minion_ast::Constraint::WatchedOr(
            exprs
                .iter()
                .map(|x| parse_expr(x.to_owned()))
                .collect::<Result<Vec<minion_ast::Constraint>, SolverError>>()?,
        )),
        conjure_ast::Expression::And(_metadata, exprs) => Ok(minion_ast::Constraint::WatchedAnd(
            exprs
                .iter()
                .map(|x| parse_expr(x.to_owned()))
                .collect::<Result<Vec<minion_ast::Constraint>, SolverError>>()?,
        )),
        conjure_ast::Expression::Eq(_metadata, a, b) => Ok(minion_ast::Constraint::Eq(
            parse_atomic_expr(*a)?,
            parse_atomic_expr(*b)?,
        )),

        conjure_ast::Expression::FlatWatchedLiteral(_metadata, name, k) => Ok(
            minion_ast::Constraint::WLiteral(parse_name(name)?, parse_literal(k)?),
        ),
        conjure_ast::Expression::MinionReify(_metadata, e, v) => Ok(minion_ast::Constraint::Reify(
            Box::new(parse_expr(*e)?),
            parse_atom(v)?,
        )),

        conjure_ast::Expression::AuxDeclaration(_metadata, name, expr) => Ok(
            minion_ast::Constraint::Eq(parse_name(name)?, parse_atomic_expr(*expr)?),
        ),

        conjure_ast::Expression::FlatMinusEq(_metadata, a, b) => Ok(
            minion_ast::Constraint::MinusEq(parse_atom(a)?, parse_atom(b)?),
        ),
        x => Err(ModelFeatureNotSupported(format!("{:?}", x))),
    }
}

fn parse_atomic_expr(expr: conjure_ast::Expression) -> Result<minion_ast::Var, SolverError> {
    let conjure_ast::Expression::Atomic(_, atom) = expr else {
        return Err(ModelInvalid(format!(
            "expected atomic expression, got {:?}",
            expr
        )));
    };

    parse_atom(atom)
}

fn parse_atoms(exprs: Vec<conjure_ast::Atom>) -> Result<Vec<minion_ast::Var>, SolverError> {
    let mut minion_vars: Vec<minion_ast::Var> = vec![];
    for expr in exprs {
        let minion_var = parse_atom(expr)?;
        minion_vars.push(minion_var);
    }
    Ok(minion_vars)
}

fn parse_atom(atom: conjure_ast::Atom) -> Result<minion_ast::Var, SolverError> {
    match atom {
        conjure_ast::Atom::Literal(l) => {
            Ok(minion_ast::Var::ConstantAsVar(parse_literal_as_int(l)?))
        }
        conjure_ast::Atom::Reference(name) => Ok(parse_name(name))?,
    }
}

fn parse_literal_as_int(k: conjure_ast::Literal) -> Result<i32, SolverError> {
    match k {
        conjure_ast::Literal::Int(n) => Ok(n),
        conjure_ast::Literal::Bool(true) => Ok(1),
        conjure_ast::Literal::Bool(false) => Ok(0),
        x => Err(ModelInvalid(format!(
            "expected a literal but got `{0:?}`",
            x
        ))),
    }
}

fn parse_literal(k: conjure_ast::Literal) -> Result<minion_ast::Constant, SolverError> {
    Ok(minion_ast::Constant::Integer(parse_literal_as_int(k)?))
}

fn parse_name(name: conjure_ast::Name) -> Result<minion_ast::Var, SolverError> {
    Ok(minion_ast::Var::NameRef(name_to_string(name)))
}
