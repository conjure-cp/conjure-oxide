use conjure_cp::ast::{Expression, Metadata, Moo, SymbolTable};
use conjure_cp::rule_engine::{
    ApplicationError, ApplicationResult, Reduction, register_rule, register_rule_set,
};

register_rule_set!("SmtBvInts", ("Base"));

#[register_rule(("SmtBvInts", 9002))]
fn fold_list_exprs_pairwise(expr: &Expression, _: &SymbolTable) -> ApplicationResult {
    match expr {
        Expression::Sum(_, vec_expr) => fold_list_pairwise(vec_expr, Expression::PairwiseSum),
        Expression::Product(_, vec_expr) => {
            fold_list_pairwise(vec_expr, Expression::PairwiseProduct)
        }
        _ => Err(ApplicationError::RuleNotApplicable),
    }
}

fn fold_list_pairwise(
    exprs: &Expression,
    op: impl Fn(Metadata, Moo<Expression>, Moo<Expression>) -> Expression,
) -> ApplicationResult {
    exprs
        .clone()
        .unwrap_list()
        .ok_or(ApplicationError::RuleNotApplicable)?
        .iter()
        .cloned()
        .reduce(|a, b| (op)(Default::default(), Moo::new(a), Moo::new(b)))
        .map(Reduction::pure)
        .ok_or(ApplicationError::RuleNotApplicable)
}
