//! Normalising rules for weighted sums.
//!
//! Weighted sums are sums in the form c1*v1 + c2*v2 + ..., where cx are literals, and vx variable
//! references.

use std::collections::BTreeMap;

use conjure_core::{
    ast::{Atom, Expression as Expr, Literal as Lit, Name, SymbolTable},
    into_matrix_expr,
    rule_engine::{ApplicationError::RuleNotApplicable, ApplicationResult, Reduction},
};
use conjure_essence_macros::essence_expr;
use conjure_rule_macros::register_rule;

/// Collects like terms in a weighted sum.
///
/// For some variable v, and constants cx,
///
/// ```plain
/// (c1 * v)  + .. + (c2 * v) + ... ~> ((c1 + c2) * v) + ...
/// ```
#[register_rule(("Base", 8400))]
fn collect_like_terms(expr: &Expr, _: &SymbolTable) -> ApplicationResult {
    let Expr::Sum(meta, exprs) = expr.clone() else {
        return Err(RuleNotApplicable);
    };

    let exprs = exprs.unwrap_list().ok_or(RuleNotApplicable)?;

    // Store:
    //  * map variable -> coefficient for weighted sum terms
    //  * a list of non-weighted sum terms

    let mut weighted_terms: BTreeMap<Name, i32> = BTreeMap::new();
    let mut other_terms: Vec<Expr> = Vec::new();

    // Assume valid terms are in form constant*variable, as reorder_product and partial_eval
    // should've already ran.

    for expr in exprs.clone() {
        match expr.clone() {
            Expr::Product(_, exprs2) => {
                match exprs2.unwrap_list().ok_or(RuleNotApplicable)?.as_slice() {
                    // todo (gs248) It would be nice to generate these destructures by macro, like `essence_expr!` but in reverse
                    // -c*v
                    [Expr::Atomic(_, Atom::Reference(name)), Expr::Neg(_, e3)] => {
                        if let Expr::Atomic(_, Atom::Literal(Lit::Int(l))) = **e3 {
                            weighted_terms
                                .insert(name.clone(), weighted_terms.get(name).unwrap_or(&0) - l);
                        } else {
                            other_terms.push(expr);
                        };
                    }

                    // c*v
                    [
                        Expr::Atomic(_, Atom::Reference(name)),
                        Expr::Atomic(_, Atom::Literal(Lit::Int(l))),
                    ] => {
                        weighted_terms
                            .insert(name.clone(), weighted_terms.get(name).unwrap_or(&0) + l);
                    }

                    // invalid
                    _ => {
                        other_terms.push(expr);
                    }
                }
            }

            // not a product
            _ => {
                other_terms.push(expr);
            }
        }
    }

    // this rule has done nothing.
    if weighted_terms.is_empty() {
        return Err(RuleNotApplicable);
    }

    let mut new_exprs = vec![];
    for (name, coefficient) in weighted_terms {
        new_exprs.push(essence_expr!(&name * &coefficient));
    }

    new_exprs.extend(other_terms);

    // no change
    if new_exprs.len() == exprs.len() {
        return Err(RuleNotApplicable);
    }

    Ok(Reduction::pure(Expr::Sum(
        meta,
        Box::new(into_matrix_expr![new_exprs]),
    )))
}
