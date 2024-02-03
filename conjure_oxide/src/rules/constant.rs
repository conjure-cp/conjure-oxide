use conjure_core::{ast::Constant as Const, ast::Expression as Expr, rule::RuleApplicationError};
use conjure_rules::register_rule;

#[register_rule]
fn apply_simplify_to_constant(expr: &Expr) -> Result<Expr, RuleApplicationError> {
    match simplify_to_constant(expr) {
        Some(c) => Ok(Expr::Constant(c)),
        None => Err(RuleApplicationError::RuleNotApplicable),
    }
}

/// Simplify an expression to a constant if possible
/// Returns:
/// `None` if the expression cannot be simplified to a constant (e.g. if it contains a variable)
/// `Some(Const)` if the expression can be simplified to a constant
pub fn simplify_to_constant(expr: &Expr) -> Option<Const> {
    match expr {
        Expr::Constant(c) => Some(c.clone()),
        Expr::Reference(_) => None,
        Expr::Eq(a, b) => {
            let a = TryInto::<i32>::try_into(simplify_to_constant(a)?).ok()?;
            let b = TryInto::<i32>::try_into(simplify_to_constant(b)?).ok()?;
            if a == b {
                Some(Const::Bool(true))
            } else {
                Some(Const::Bool(false))
            }
        }
        _ => None,
    }
}

// fn unwrap_consts(v: Vec<Const>) -> Option<Vec<T>> {
//     v.iter().map(|c| T.try_from(c))
// }

// fn unwrap_vec(exprs: Vec<Expr>) -> Option<Vec<Const>> {
//     exprs
//         .iter()
//         .map(simplify_to_constant)
//         .into_iter()
//         .collect::<Option<Vec<Const>>>()
// }
