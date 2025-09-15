//! Generic normalising rules for associative-commutative operators.

use std::collections::VecDeque;
use std::mem::Discriminant;

use conjure_cp::ast::{Expression as Expr, SymbolTable};
use conjure_cp::rule_engine::{
    ApplicationError::RuleNotApplicable, ApplicationResult, Reduction, register_rule,
};
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
fn normalise_associative_commutative(expr: &Expr, _: &SymbolTable) -> ApplicationResult {
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

        let child_vecs: VecDeque<Vec<Expr>> = expr.children_bi();

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

    let child_vecs: VecDeque<Vec<Expr>> = expr.children_bi();
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
