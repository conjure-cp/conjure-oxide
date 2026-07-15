//! Normalising rules for weighted sums.
//!
//! Weighted sums are sums in the form c1*v1 + c2*v2 + ..., where cx are literals, and vx variable
//! references.

use std::collections::{BTreeMap, BTreeSet};

use conjure_cp::ast::{AbstractLiteral, Reference};
use conjure_cp::essence_expr;
use conjure_cp::rule_engine::register_rule;
use conjure_cp::{
    ast::Metadata,
    ast::{Atom, Expression as Expr, IntVal, Literal as Lit, Moo, Range, SymbolTable},
    into_matrix_expr,
    rule_engine::{ApplicationError::RuleNotApplicable, ApplicationResult, RuleEffect},
};

/// Collects like terms in a weighted sum.
///
/// For some variable v, and constants cx,
///
/// ```plain
/// (c1 * v)  + .. + (c2 * v) + ... ~> ((c1 + c2) * v) + ...
/// ```
#[register_rule("Base", 8400, [Sum])]
fn collect_like_terms(expr: &Expr, _: &SymbolTable) -> ApplicationResult {
    let Expr::Sum(meta, exprs) = expr else {
        return Err(RuleNotApplicable);
    };
    let exprs = expr_list_elements(exprs.as_ref()).ok_or(RuleNotApplicable)?;

    if !has_duplicate_weighted_reference(exprs) {
        return Err(RuleNotApplicable);
    }

    // Store:
    //  * map variable -> coefficient for weighted sum terms
    //  * a list of non-weighted sum terms

    #[allow(clippy::mutable_key_type)]
    let mut weighted_terms: BTreeMap<Reference, i32> = BTreeMap::new();
    let mut other_terms: Vec<Expr> = Vec::new();

    // Assume valid terms are in form constant*variable, as reorder_product and partial_eval
    // should've already ran.

    for expr in exprs.iter() {
        if let Some((re, coefficient)) = weighted_term(expr) {
            let curr_weight = weighted_terms.get(re).unwrap_or(&0);
            weighted_terms.insert(re.clone(), curr_weight + coefficient);
        } else {
            other_terms.push(expr.clone());
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

    Ok(RuleEffect::pure(Expr::Sum(
        meta.clone(),
        Moo::new(into_matrix_expr![new_exprs]),
    )))
}

fn has_duplicate_weighted_reference(exprs: &[Expr]) -> bool {
    let mut seen = BTreeSet::new();
    for expr in exprs {
        let Some((reference, _)) = weighted_term(expr) else {
            continue;
        };
        if !seen.insert(reference.id()) {
            return true;
        }
    }

    false
}

fn weighted_term(expr: &Expr) -> Option<(&Reference, i32)> {
    let Expr::Product(_, exprs) = expr else {
        return None;
    };

    match expr_list_elements(exprs.as_ref())? {
        // todo (gs248) It would be nice to generate these destructures by macro, like `essence_expr!` but in reverse
        // -c*v
        [Expr::Atomic(_, Atom::Reference(re)), Expr::Neg(_, e3)] => {
            let Expr::Atomic(_, Atom::Literal(Lit::Int(l))) = e3.as_ref() else {
                return None;
            };
            Some((re, -*l))
        }

        // c*v
        [
            Expr::Atomic(_, Atom::Reference(re)),
            Expr::Atomic(_, Atom::Literal(Lit::Int(l))),
        ] => Some((re, *l)),

        _ => None,
    }
}

fn expr_list_elements(expr: &Expr) -> Option<&[Expr]> {
    match expr {
        Expr::AbstractLiteral(_, AbstractLiteral::Matrix(elems, domain))
            if domain.as_int().is_some_and(|ranges| {
                matches!(ranges.as_slice(), [Range::UnboundedR(IntVal::Const(1))])
            }) =>
        {
            Some(elems)
        }
        _ => None,
    }
}
