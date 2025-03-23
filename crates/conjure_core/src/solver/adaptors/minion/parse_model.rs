//! Parse / `load_model` step of running Minion.

use std::cell::Ref;

use itertools::Itertools as _;
use minion_ast::Model as MinionModel;
use minion_rs::ast as minion_ast;
use minion_rs::error::MinionError;
use minion_rs::{get_from_table, run_minion};

use crate::ast as conjure_ast;
use crate::ast::Declaration;
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
    if let Some(ref vars) = conjure_model.search_order {
        // add search vars in order first
        for name in vars {
            let decl = conjure_model
                .as_submodel()
                .symbols()
                .lookup(name)
                .expect("search var should exist");
            let var = decl.as_var().expect("search var should be a var");

            load_var(name, var, true, minion_model)?;
        }

        // then add the rest as non-search vars
        for (name, decl) in conjure_model
            .as_submodel()
            .symbols()
            .clone()
            .into_iter_local()
        {
            // search var - already added
            if vars.contains(&name) {
                continue;
            };

            let Some(var) = decl.as_var() else {
                continue;
            }; // ignore lettings, etc.
               //

            // this variable has representations, so ignore it
            if !conjure_model
                .as_submodel()
                .symbols()
                .representations_for(&name)
                .is_none_or(|x| x.is_empty())
            {
                continue;
            };

            load_var(&name, var, false, minion_model)?;
        }
    } else {
        for (name, decl) in conjure_model
            .as_submodel()
            .symbols()
            .clone()
            .into_iter_local()
        {
            let Some(var) = decl.as_var() else {
                continue;
            }; // ignore lettings, etc.
               //

            // this variable has representations, so ignore it
            if !conjure_model
                .as_submodel()
                .symbols()
                .representations_for(&name)
                .is_none_or(|x| x.is_empty())
            {
                continue;
            };

            let is_search_var = !matches!(name, conjure_ast::Name::MachineName(_));

            load_var(&name, var, is_search_var, minion_model)?;
        }
    }
    Ok(())
}

/// Loads a single variable into `minion_model`
fn load_var(
    name: &conjure_ast::Name,
    var: &conjure_ast::DecisionVariable,
    search_var: bool,
    minion_model: &mut MinionModel,
) -> Result<(), SolverError> {
    match &var.domain {
        conjure_ast::Domain::IntDomain(ranges) => {
            load_intdomain_var(name, ranges, search_var, minion_model)
        }
        conjure_ast::Domain::BoolDomain => load_booldomain_var(name, search_var, minion_model),
        x => Err(ModelFeatureNotSupported(format!("{:?}", x))),
    }
}

/// Loads a variable with domain IntDomain into `minion_model`
fn load_intdomain_var(
    name: &conjure_ast::Name,
    ranges: &[conjure_ast::Range<i32>],
    search_var: bool,
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

    let domain = minion_ast::VarDomain::Bound(low, high);
    let str_name = str_name.to_owned();

    if search_var {
        _try_add_var(str_name, domain, minion_model)
    } else {
        _try_add_aux_var(str_name, domain, minion_model)
    }
}

/// Loads a variable with domain BoolDomain into `minion_model`
fn load_booldomain_var(
    name: &conjure_ast::Name,
    search_var: bool,
    minion_model: &mut MinionModel,
) -> Result<(), SolverError> {
    let str_name = name_to_string(name.to_owned());
    let domain = minion_ast::VarDomain::Bool;
    if search_var {
        _try_add_var(str_name, domain, minion_model)
    } else {
        _try_add_aux_var(str_name, domain, minion_model)
    }
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

fn _try_add_aux_var(
    name: minion_ast::VarName,
    domain: minion_ast::VarDomain,
    minion_model: &mut MinionModel,
) -> Result<(), SolverError> {
    minion_model
        .named_variables
        .add_aux_var(name.clone(), domain)
        .ok_or(ModelInvalid(format!(
            "variable {:?} is defined twice",
            name
        )))
}

fn name_to_string(name: conjure_ast::Name) -> String {
    match name {
        // print machine names in a custom, easier to regex, way.
        conjure_ast::Name::MachineName(x) => format!("__conjure_machine_name_{}", x),
        conjure_ast::Name::RepresentedName(name, rule, suffix) => {
            let name = name_to_string(*name);
            format!("__conjure_represented_name##{name}##{rule}___{suffix}")
        }
        x => format!("{x}"),
    }
}

/// Loads the constraints into `minion_model`.
fn load_constraints(
    conjure_model: &ConjureModel,
    minion_model: &mut MinionModel,
) -> Result<(), SolverError> {
    for expr in conjure_model.as_submodel().constraints().iter() {
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
        conjure_ast::Expression::FlatAllDiff(_metadata, atoms) => {
            Ok(minion_ast::Constraint::AllDiff(parse_atoms(atoms)?))
        }
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
        conjure_ast::Expression::MinionWInIntervalSet(_metadata, a, xs) => {
            Ok(minion_ast::Constraint::WInIntervalSet(
                parse_atom(a)?,
                xs.into_iter()
                    .map(minion_ast::Constant::Integer)
                    .collect_vec(),
            ))
        }

        conjure_ast::Expression::MinionElementOne(_, vec, i, e) => Ok(
            minion_ast::Constraint::ElementOne(parse_atoms(vec)?, parse_atom(i)?, parse_atom(e)?),
        ),

        conjure_ast::Expression::Or(_metadata, e) => Ok(minion_ast::Constraint::WatchedOr(
            e.unwrap_matrix_unchecked()
                .ok_or_else(|| {
                    SolverError::ModelFeatureNotSupported(
                        "The inside of an or expression is not a matrix.".to_string(),
                    )
                })?
                .0
                .iter()
                .map(|x| parse_expr(x.to_owned()))
                .collect::<Result<Vec<minion_ast::Constraint>, SolverError>>()?,
        )),
        conjure_ast::Expression::And(_metadata, e) => Ok(minion_ast::Constraint::WatchedAnd(
            e.unwrap_matrix_unchecked()
                .ok_or_else(|| {
                    SolverError::ModelFeatureNotSupported(
                        "The inside of an and expression is not a matrix.".to_string(),
                    )
                })?
                .0
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

        conjure_ast::Expression::MinionReifyImply(_metadata, e, v) => Ok(
            minion_ast::Constraint::ReifyImply(Box::new(parse_expr(*e)?), parse_atom(v)?),
        ),

        conjure_ast::Expression::AuxDeclaration(_metadata, name, expr) => Ok(
            minion_ast::Constraint::Eq(parse_name(name)?, parse_atomic_expr(*expr)?),
        ),

        conjure_ast::Expression::FlatMinusEq(_metadata, a, b) => Ok(
            minion_ast::Constraint::MinusEq(parse_atom(a)?, parse_atom(b)?),
        ),

        conjure_ast::Expression::FlatProductEq(_metadata, a, b, c) => Ok(
            minion_ast::Constraint::Product((parse_atom(a)?, parse_atom(b)?), parse_atom(c)?),
        ),
        conjure_ast::Expression::FlatWeightedSumLeq(_metadata, coeffs, vars, total) => {
            Ok(minion_ast::Constraint::WeightedSumLeq(
                parse_literals(coeffs)?,
                parse_atoms(vars)?,
                parse_atom(total)?,
            ))
        }
        conjure_ast::Expression::FlatWeightedSumGeq(_metadata, coeffs, vars, total) => {
            Ok(minion_ast::Constraint::WeightedSumGeq(
                parse_literals(coeffs)?,
                parse_atoms(vars)?,
                parse_atom(total)?,
            ))
        }
        conjure_ast::Expression::FlatAbsEq(_metadata, x, y) => {
            Ok(minion_ast::Constraint::Abs(parse_atom(x)?, parse_atom(y)?))
        }
        conjure_ast::Expression::MinionPow(_, x, y, z) => Ok(minion_ast::Constraint::Pow(
            (parse_atom(x)?, parse_atom(y)?),
            parse_atom(z)?,
        )),
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

        x => Err(ModelFeatureNotSupported(format!(
            "expected a literal or a reference but got `{0}`",
            x
        ))),
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

fn parse_literals(
    literals: Vec<conjure_ast::Literal>,
) -> Result<Vec<minion_ast::Constant>, SolverError> {
    let mut minion_constants: Vec<minion_ast::Constant> = vec![];
    for literal in literals {
        let minion_constant = parse_literal(literal)?;
        minion_constants.push(minion_constant);
    }
    Ok(minion_constants)
}

fn parse_literal(k: conjure_ast::Literal) -> Result<minion_ast::Constant, SolverError> {
    Ok(minion_ast::Constant::Integer(parse_literal_as_int(k)?))
}

fn parse_name(name: conjure_ast::Name) -> Result<minion_ast::Var, SolverError> {
    Ok(minion_ast::Var::NameRef(name_to_string(name)))
}
