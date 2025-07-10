//! Normalising rules for `Product`

use std::iter;

use conjure_rule_macros::register_rule;

use conjure_core::{
    ast::{Atom, Expression as Expr, Literal as Lit, SymbolTable},
    into_matrix_expr,
    metadata::Metadata,
    rule_engine::{ApplicationError::RuleNotApplicable, ApplicationResult, Reduction},
};

/// Reorders a product expression.
///
/// The resulting product will have the following order:
///
/// 1. Constant coefficients
/// 2. Variables
/// 3. Compound terms
///
/// The order of items within each category is undefined.
///
/// # Justification
///
/// Having a canonical ordering here is helpful in identifying weighted sums: 2x + 3y + 4d + ....
#[register_rule(("Base", 8800))]
fn reorder_product(expr: &Expr, _: &SymbolTable) -> ApplicationResult {
    let Expr::Product(meta, factors) = expr.clone() else {
        return Err(RuleNotApplicable);
    };

    let mut constant_coefficients: Vec<Expr> = vec![];
    let mut variables: Vec<Expr> = vec![];
    let mut compound_exprs: Vec<Expr> = vec![];

    let factors = factors.unwrap_list().ok_or(RuleNotApplicable)?;

    for expr in factors.clone() {
        match expr {
            Expr::Atomic(_, Atom::Literal(_)) => {
                constant_coefficients.push(expr);
            }
            Expr::Atomic(_, Atom::Reference(_)) => {
                variables.push(expr);
            }

            // -1 is a constant coefficient
            Expr::Neg(_, ref expr2) if matches!(**expr2, Expr::Atomic(_, Atom::Literal(_))) => {
                constant_coefficients.push(expr);
            }

            // -x === -1 * x
            Expr::Neg(_, expr2) if matches!(*expr2, Expr::Atomic(_, Atom::Reference(_))) => {
                constant_coefficients
                    .push(Expr::Atomic(Metadata::new(), Atom::Literal(Lit::Int(-1))));
                variables.push(*expr2);
            }

            _ => {
                compound_exprs.push(expr);
            }
        }
    }

    constant_coefficients.extend(variables);
    constant_coefficients.extend(compound_exprs);

    // check if we have actually done anything
    // TODO: check order before doing all this
    let mut changed: bool = false;
    for (e1, e2) in iter::zip(factors, constant_coefficients.clone()) {
        if e1 != e2 {
            changed = true;
            break;
        }
    }

    if !changed {
        return Err(RuleNotApplicable);
    }

    Ok(Reduction::pure(Expr::Product(
        meta,
        Box::new(into_matrix_expr!(constant_coefficients)),
    )))
}

/// Removes products with a single argument.
///
/// ```text
/// product([a]) ~> a
/// ```
///
#[register_rule(("Base", 8800))]
fn remove_unit_vector_products(expr: &Expr, _: &SymbolTable) -> ApplicationResult {
    match expr {
        Expr::Product(_, mat) => {
            let list = mat.clone().unwrap_list().ok_or(RuleNotApplicable)?;
            if list.len() == 1 {
                return Ok(Reduction::pure(list[0].clone()));
            }
            Err(RuleNotApplicable)
        }
        _ => Err(RuleNotApplicable),
    }
}
