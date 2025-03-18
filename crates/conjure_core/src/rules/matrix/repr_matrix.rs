use conjure_core::ast::Expression as Expr;
use conjure_core::ast::{matrix, SymbolTable};
use conjure_core::rule_engine::{
    register_rule, ApplicationError::RuleNotApplicable, ApplicationResult, Reduction,
};
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

    let Expr::Atomic(_, Atom::Reference(Name::WithRepresentation(name, reprs))) = &**subject else {
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

    let Some(Domain::DomainMatrix(_, _)) = decl.domain() else {
        return Err(RuleNotApplicable);
    };

    let mut indices_as_lits: Vec<Literal> = vec![];

    for index in indices {
        let index = index.clone().to_literal().ok_or(RuleNotApplicable)?;
        indices_as_lits.push(index);
    }

    let indices_as_name = Name::RepresentedName(
        name.clone(),
        "matrix_to_atom".into(),
        indices_as_lits.iter().join("_"),
    );

    let subject = repr.expression_down(symbols)?[&indices_as_name].clone();

    Ok(Reduction::pure(subject))
}

/// Using the `matrix_to_atom` representation rule, rewrite matrix slicing.
#[register_rule(("Base", 2000))]
fn slice_matrix_to_atom(expr: &Expr, symbols: &SymbolTable) -> ApplicationResult {
    let Expr::SafeSlice(_, subject, indices) = expr else {
        return Err(RuleNotApplicable);
    };

    let Expr::Atomic(_, Atom::Reference(Name::WithRepresentation(name, reprs))) = &**subject else {
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

    let mut indices_as_lits: Vec<Option<Literal>> = vec![];
    let mut hole_dim: i32 = -1;
    for (i, index) in indices.iter().enumerate() {
        match index {
            Some(e) => {
                let lit = e.clone().to_literal().ok_or(RuleNotApplicable)?;
                indices_as_lits.push(Some(lit.clone()));
            }
            None => {
                indices_as_lits.push(None);
                assert_eq!(hole_dim, -1);
                hole_dim = i as _;
            }
        }
    }

    assert_ne!(hole_dim, -1);

    let repr_values = repr.expression_down(symbols)?;

    let slice = index_domains[hole_dim as usize]
        .values()
        .expect("index domain should be finite and enumerable")
        .into_iter()
        .map(|i| {
            let mut indices_as_lits = indices_as_lits.clone();
            indices_as_lits[hole_dim as usize] = Some(i);
            let name = Name::RepresentedName(
                name.clone(),
                "matrix_to_atom".into(),
                indices_as_lits.into_iter().map(|x| x.unwrap()).join("_"),
            );
            repr_values[&name].clone()
        })
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
        let Expr::Atomic(_, Atom::Reference(Name::WithRepresentation(name, reprs))) = child else {
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

        let flat_values = matrix::enumerate_indices(index_domains.clone())
            .map(|i| {
                matrix_values[&Name::RepresentedName(
                    name.clone(),
                    "matrix_to_atom".into(),
                    i.iter().join("_"),
                )]
                    .clone()
            })
            .collect_vec();
        return Ok(Reduction::pure(ctx(into_matrix_expr![flat_values])));
    }

    Err(RuleNotApplicable)
}
