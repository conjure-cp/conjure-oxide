use conjure_core::{ast::Constant as Const, ast::Expression as Expr, rule::RuleApplicationError};
use conjure_rules::register_rule;

/************************************************************************/
/*        Rules for translating to Minion-supported constraints         */
/************************************************************************/

#[register_rule]
fn flatten_sum_geq(expr: &Expr) -> Result<Expr, RuleApplicationError> {
    match expr {
        Expr::Geq(a, b) => {
            let exprs = match a.as_ref() {
                Expr::Sum(exprs) => Ok(exprs),
                _ => Err(RuleApplicationError::RuleNotApplicable),
            }?;
            Ok(Expr::SumGeq(exprs.clone(), b.clone()))
        }
        _ => Err(RuleApplicationError::RuleNotApplicable),
    }
}

#[register_rule]
fn sum_leq_to_sumleq(expr: &Expr) -> Result<Expr, RuleApplicationError> {
    match expr {
        Expr::Leq(a, b) => {
            let exprs = match a.as_ref() {
                Expr::Sum(exprs) => Ok(exprs),
                _ => Err(RuleApplicationError::RuleNotApplicable),
            }?;
            Ok(Expr::SumLeq(exprs.clone(), b.clone()))
        }
        _ => Err(RuleApplicationError::RuleNotApplicable),
    }
}

#[register_rule]
fn lt_to_ineq(expr: &Expr) -> Result<Expr, RuleApplicationError> {
    match expr {
        Expr::Lt(a, b) => Ok(Expr::Ineq(
            a.clone(),
            b.clone(),
            Box::new(Expr::Constant(Const::Int(-1))),
        )),
        _ => Err(RuleApplicationError::RuleNotApplicable),
    }
}
