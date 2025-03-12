use conjure_core::ast::Expression as Expr;
use conjure_core::ast::SymbolTable;
use conjure_core::rule_engine::{
    register_rule, ApplicationError::RuleNotApplicable, ApplicationResult, Reduction,
};
use itertools::izip;
use itertools::Itertools;
use uniplate::Uniplate;

use crate::ast::Atom;
use crate::ast::Domain;
use crate::ast::Literal;
use crate::ast::Name;
use crate::into_matrix_expr;

/// Using the `matrix_to_atom`  representation rule, rewrite matrix indexing.
#[register_rule(("Base", 2000))]
fn index_matrix_to_atom(expr: &Expr, symbols: &SymbolTable) -> ApplicationResult {
    let Expr::SafeIndex(_, subject, indices) = expr else {
        return Err(RuleNotApplicable);
    };

    let Expr::Atomic(_, Atom::Reference(Name::HasRepresentation(name, reprs))) = &**subject else {
        return Err(RuleNotApplicable);
    };

    if reprs.first().is_none_or(|x| x.as_str() != "matrix_to_atom") {
        return Err(RuleNotApplicable);
    }

    let decl = symbols.lookup(name).unwrap();
    let repr = symbols
        .get_representation(name, &["matrix_to_atom"])
        .unwrap()[0]
        .clone();

    let Some(Domain::DomainMatrix(_, index_domains)) = decl.domain() else {
        return Err(RuleNotApplicable);
    };

    let mut lit_indicies: Vec<Literal> = vec![];

    for index in indices {
        let Expr::Atomic(_, Atom::Literal(lit)) = index else {
            return Err(RuleNotApplicable);
        };
        lit_indicies.push(lit.clone());
    }

    // all the possible indices in this matrix
    let matrix_indices = index_domains
        .iter()
        .map(|domain| domain.values().expect("matrix index domains to be finite"))
        .multi_cartesian_product()
        .collect_vec();

    // TODO: gently fail
    let flat_index = matrix_indices
        .iter()
        .position(|x| x == &lit_indicies)
        .unwrap();

    let subject = repr.expression_down(symbols)?[flat_index].clone();

    Ok(Reduction::pure(subject))
}

/// Using the `matrix_to_atom` representation rule, rewrite matrix slicing.
#[register_rule(("Base", 2000))]
fn slice_matrix_to_atom(expr: &Expr, symbols: &SymbolTable) -> ApplicationResult {
    let Expr::SafeSlice(_, subject, indices) = expr else {
        return Err(RuleNotApplicable);
    };

    let Expr::Atomic(_, Atom::Reference(Name::HasRepresentation(name, reprs))) = &**subject else {
        return Err(RuleNotApplicable);
    };

    if reprs.first().is_none_or(|x| x.as_str() != "matrix_to_atom") {
        return Err(RuleNotApplicable);
    }

    let decl = symbols.lookup(name).unwrap();
    let repr = symbols
        .get_representation(name, &["matrix_to_atom"])
        .unwrap()[0]
        .clone();

    let Some(Domain::DomainMatrix(_, index_domains)) = decl.domain() else {
        return Err(RuleNotApplicable);
    };

    let mut lit_indices: Vec<Option<Literal>> = vec![];
    for index in indices {
        match index {
            Some(Expr::Atomic(_, Atom::Literal(lit))) => {
                lit_indices.push(Some(lit.clone()));
            }
            None => {
                lit_indices.push(None);
            }
            _ => {
                return Err(RuleNotApplicable);
            }
        }
    }

    // all the possible indices in this matrix
    let matrix_indices = index_domains
        .iter()
        .map(|domain| domain.values().expect("matrix index domains to be finite"))
        .multi_cartesian_product()
        .collect_vec();

    let repr_expressions = repr.expression_down(symbols)?;

    let slice = matrix_indices
        .iter()
        .enumerate()
        .filter(|(_, is)| {
            izip!(&lit_indices, *is).all(|(usr_idx, i)| match usr_idx {
                None => true,
                Some(j) => j == i,
            })
        })
        .map(|(i, _)| i)
        .map(|i| repr_expressions[i].clone())
        .collect_vec();

    let new_expr = into_matrix_expr!(slice);

    Ok(Reduction::pure(new_expr))
}

/// Converts a reference to a 1d-matrix not contained within an indexing or slicing expression to its atoms.
#[register_rule(("Base", 2000))]
fn matrix_ref_to_atom(expr: &Expr, symbols: &SymbolTable) -> ApplicationResult {
    if let Expr::SafeSlice(_, _, _)
    | Expr::UnsafeSlice(_, _, _)
    | Expr::SafeIndex(_, _, _)
    | Expr::UnsafeIndex(_, _, _) = expr
    {
        return Err(RuleNotApplicable);
    };

    for (child, ctx) in expr.holes() {
        let Expr::Atomic(_, Atom::Reference(Name::HasRepresentation(name, reprs))) = child else {
            continue;
        };

        if reprs.first().is_none_or(|x| x.as_str() != "matrix_to_atom") {
            continue;
        }

        let decl = symbols.lookup(name.as_ref()).unwrap();
        let repr = symbols
            .get_representation(name.as_ref(), &["matrix_to_atom"])
            .unwrap()[0]
            .clone();

        let Some(Domain::DomainMatrix(_, index_domains)) = decl.domain() else {
            continue;
        };

        if index_domains.len() > 1 {
            continue;
        }

        let Ok(matrix_values) = repr.expression_down(symbols) else {
            continue;
        };

        return Ok(Reduction::pure(ctx(into_matrix_expr![matrix_values])));
    }

    Err(RuleNotApplicable)
}
