//! Normalising rules for `Sum`

use conjure_core::ast::Expression as Expr;
use conjure_core::rule_engine::{register_rule, ApplicationError, ApplicationResult, Reduction};
use conjure_core::Model;

use Expr::*;

/// Removes sums with a single argument.
///
/// ```text
/// sum([a]) ~> a
/// ```
#[register_rule(("Base", 8800))]
fn remove_unit_vector_sum(expr: &Expr, _: &Model) -> ApplicationResult {
    match expr {
        Sum(_, exprs) if (exprs.len() == 1) => Ok(Reduction::pure(exprs[0].clone())),
        _ => Err(ApplicationError::RuleNotApplicable),
    }
}

/// Unwraps nested sums
///
/// ```text
/// sum(sum(a, b), c) ~> sum(a, b, c)
/// ```
#[register_rule(("Base", 8800))]
pub fn unwrap_nested_sum(expr: &Expr, _: &Model) -> ApplicationResult {
    match expr {
        Sum(metadata, exprs) => {
            let mut new_exprs = Vec::new();
            let mut changed = false;
            for e in exprs {
                match e {
                    Sum(_, sub_exprs) => {
                        changed = true;
                        for e in sub_exprs {
                            new_exprs.push(e.clone());
                        }
                    }
                    _ => new_exprs.push(e.clone()),
                }
            }
            if !changed {
                return Err(ApplicationError::RuleNotApplicable);
            }
            Ok(Reduction::pure(Sum(metadata.clone_dirty(), new_exprs)))
        }
        _ => Err(ApplicationError::RuleNotApplicable),
    }
}
