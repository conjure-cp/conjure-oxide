//! Generic normalising rules for associative-commutative operators.

use std::mem::Discriminant;

use crate::shared::utils::{single_vec_child, with_single_vec_child};
use conjure_cp::ast::{AbstractLiteral, Expression as Expr, SymbolTable};
use conjure_cp::rule_engine::{
    ApplicationError::RuleNotApplicable, ApplicationResult, RuleEffect, register_rule,
};

/// Normalises associative_commutative operations.
///
/// For now, this just removes nested expressions by associativity.
///
/// ```text
/// v(v(a,b,...),c,d,...) ~> v(a,b,c,d)
/// v(a,b,[c,d],...)      ~> v(a,b,c,d)
/// where v is an AC vector operator
/// ```
///
/// The second form arises when a comprehension standing for several terms is expanded in place:
/// the expansion lands as a matrix literal among the operands, and an AC operator's operand list
/// is flat by definition.
#[register_rule("Base", 8900, [And, Or, Product, Sum])]
fn normalise_associative_commutative(expr: &Expr, _: &SymbolTable) -> ApplicationResult {
    if !expr.is_associative_commutative_operator() {
        return Err(RuleNotApplicable);
    }

    if !has_direct_nested_ac_child(expr) {
        return Err(RuleNotApplicable);
    }

    // remove nesting deeply
    fn recurse_deeply(
        root_discriminant: Discriminant<Expr>,
        expr: Expr,
        changed: &mut bool,
    ) -> Vec<Expr> {
        // A matrix literal among the operands is spliced in, whatever the operator.
        if let Expr::AbstractLiteral(_, AbstractLiteral::Matrix(children, _)) = &expr {
            *changed = true;
            return children
                .clone()
                .into_iter()
                .flat_map(|child| recurse_deeply(root_discriminant, child, changed))
                .collect();
        }

        // if expr a different expression type, stop recursing
        if std::mem::discriminant(&expr) != root_discriminant {
            return vec![expr];
        }

        let Some(children) = single_vec_child(&expr) else {
            return vec![expr];
        };
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

    if single_vec_child(expr).is_none() {
        return Err(RuleNotApplicable);
    }

    let mut changed = false;
    let new_children = recurse_deeply(std::mem::discriminant(expr), expr.clone(), &mut changed);

    if !changed {
        return Err(RuleNotApplicable);
    }

    let new_expr = with_single_vec_child(expr, new_children);

    Ok(RuleEffect::pure(new_expr))
}

fn has_direct_nested_ac_child(expr: &Expr) -> bool {
    let root_discriminant = std::mem::discriminant(expr);
    ac_children(expr).is_some_and(|children| {
        children.iter().any(|child| {
            std::mem::discriminant(child) == root_discriminant
                || matches!(
                    child,
                    Expr::AbstractLiteral(_, AbstractLiteral::Matrix(_, _))
                )
        })
    })
}

fn ac_children(expr: &Expr) -> Option<&[Expr]> {
    let matrix = match expr {
        Expr::And(_, matrix)
        | Expr::Or(_, matrix)
        | Expr::Product(_, matrix)
        | Expr::Sum(_, matrix) => matrix.as_ref(),
        _ => return None,
    };

    match matrix {
        Expr::AbstractLiteral(_, AbstractLiteral::Matrix(children, _)) => Some(children),
        _ => None,
    }
}
