//! Generic normalising rules for associative-commutative operators.

use std::mem::Discriminant;

use conjure_core::ast::Expression as Expr;
use conjure_core::rule_engine::{
    register_rule, ApplicationError::RuleNotApplicable, ApplicationResult, Reduction,
};
use conjure_core::Model;
use uniplate::Biplate;

/// Normalises associative_commutative operations.
///
/// For now, this just removes nested expressions by associativity.
///
/// ```text
/// v(v(a,b,...),c,d,...) ~> v(a,b,c,d)
/// where v is an AC vector operator
/// ```
#[register_rule(("Base", 8900))]
fn normalise_associative_commutative(expr: &Expr, _: &Model) -> ApplicationResult {
    if !expr.is_associative_commutative_operator() {
        return Err(RuleNotApplicable);
    }

    // remove nesting deeply
    fn recurse_deeply(
        root_discriminant: Discriminant<Expr>,
        expr: Expr,
        changed: &mut bool,
    ) -> Vec<Expr> {
        // if expr a different expression type, stop recursing
        if std::mem::discriminant(&expr) != root_discriminant {
            return vec![expr];
        }

        let child_vecs = <_ as Biplate<Vec<Expr>>>::children_bi(&expr);

        // empty expression
        if child_vecs.is_empty() {
            return vec![expr];
        }

        // go deeper
        let children = child_vecs[0].clone();
        let old_len = children.len();

        let new_children = children
            .into_iter()
            .flat_map(|child| recurse_deeply(root_discriminant, child, changed))
            .collect::<Vec<_>>();
        if new_children.len() != old_len {
            *changed = true;
        }

        new_children
    }

    let child_vecs = <_ as Biplate<Vec<Expr>>>::children_bi(expr);
    if child_vecs.is_empty() {
        return Err(RuleNotApplicable);
    }

    let mut changed = false;
    let new_children = recurse_deeply(std::mem::discriminant(expr), expr.clone(), &mut changed);

    if !changed {
        return Err(RuleNotApplicable);
    }

    let new_expr = expr.with_children_bi(vec![new_children].into());

    Ok(Reduction::pure(new_expr))
}
