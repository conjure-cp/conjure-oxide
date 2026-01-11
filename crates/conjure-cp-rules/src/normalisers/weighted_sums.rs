//! Normalising rules for weighted sums.
//!
//! Weighted sums are sums in the form c1*v1 + c2*v2 + ..., where cx are literals, and vx variable
//! references.

use std::collections::BTreeMap;

use conjure_cp::ast::Reference;
use conjure_cp::essence_expr;
use conjure_cp::rule_engine::register_rule;
use conjure_cp::{
    ast::Metadata,
    ast::{Atom, Expression as Expr, Literal as Lit, Moo, SymbolTable},
    into_matrix_expr,
    rule_engine::{ApplicationError::RuleNotApplicable, ApplicationResult, Reduction},
};

/// Collects like terms in a weighted sum.
///
/// For some variable v, and constants cx,
///
/// ```plain
/// (c1 * v)  + .. + (c2 * v) + ... ~> ((c1 + c2) * v) + ...
/// ```
#[register_rule(("Base", 8400))]
fn collect_like_terms(expr: &Expr, _: &SymbolTable) -> ApplicationResult {
    let Expr::Sum(meta, exprs) = expr else {
        return Err(RuleNotApplicable);
    };
    let exprs = exprs.unwrap_list().ok_or(RuleNotApplicable)?;

    // Store:
    //  * map variable -> coefficient for weighted sum terms
    //  * a list of non-weighted sum terms

    #[allow(clippy::mutable_key_type)]
    let mut weighted_terms: BTreeMap<Reference, i32> = BTreeMap::new();
    let mut other_terms: Vec<Expr> = Vec::new();

    // Assume valid terms are in form constant*variable, as reorder_product and partial_eval
    // should've already ran.

    for expr in exprs.iter() {
        match expr {
            Expr::Product(_, exprs2) => {
                match exprs2.unwrap_list().ok_or(RuleNotApplicable)?.as_slice() {
                    // todo (gs248) It would be nice to generate these destructures by macro, like `essence_expr!` but in reverse
                    // -c*v
                    [Expr::Atomic(_, Atom::Reference(re)), Expr::Neg(_, e3)] => {
                        if let Expr::Atomic(_, Atom::Literal(Lit::Int(l))) = **e3 {
                            let curr_weight = weighted_terms.get(re).unwrap_or(&0);
                            weighted_terms.insert(re.clone(), curr_weight - l);
                        } else {
                            other_terms.push(expr.clone());
                        };
                    }

                    // c*v
                    [
                        Expr::Atomic(_, Atom::Reference(re)),
                        Expr::Atomic(_, Atom::Literal(Lit::Int(l))),
                    ] => {
                        let curr_weight = weighted_terms.get(re).unwrap_or(&0);
                        weighted_terms.insert(re.clone(), curr_weight + l);
                    }

                    // invalid
                    _ => {
                        other_terms.push(expr.clone());
                    }
                }
            }

            // not a product
            _ => {
                other_terms.push(expr.clone());
            }
        }
    }

    // this rule has done nothing.
    if weighted_terms.is_empty() {
        return Err(RuleNotApplicable);
    }

    let mut new_exprs = vec![];
    for (re, coefficient) in weighted_terms {
        let atom = Expr::Atomic(Metadata::new(), Atom::Reference(re));
        new_exprs.push(essence_expr!(&atom * &coefficient));
    }

    new_exprs.extend(other_terms);

    // no change
    if new_exprs.len() == exprs.len() {
        return Err(RuleNotApplicable);
    }

    Ok(Reduction::pure(Expr::Sum(
        meta.clone(),
        Moo::new(into_matrix_expr![new_exprs]),
    )))
}
