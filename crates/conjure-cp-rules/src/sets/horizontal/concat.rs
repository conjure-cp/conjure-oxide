// rules for concatenations of subsetEq with intersect and union
// analogous rules for subset, supset and supsetEq are not needed because these are converted to subsetEq first.
use conjure_cp::ast::Metadata;
use conjure_cp::ast::{Expression as Expr, Moo, ReturnType, SymbolTable, Typeable};
use conjure_cp::matrix_expr;
use conjure_cp::rule_engine::Reduction;
use conjure_cp::rule_engine::{
    ApplicationError::RuleNotApplicable, ApplicationResult, register_rule,
};

// A subsetEq (B intersect C) -> A subsetEq B /\ A subsetEq C
#[register_rule(("Base", 8700))]
fn subseteq_intersect(expr: &Expr, _: &SymbolTable) -> ApplicationResult {
    match expr {
        Expr::SubsetEq(_, a, rhs) if matches!(a.as_ref().return_type(), ReturnType::Set(_)) => {
            match rhs.as_ref() {
                Expr::Intersect(_, b, c)
                    if matches!(b.as_ref().return_type(), ReturnType::Set(_))
                        && matches!(c.as_ref().return_type(), ReturnType::Set(_)) =>
                {
                    let expr1 = Expr::SubsetEq(Metadata::new(), a.clone(), b.clone());
                    let expr2 = Expr::SubsetEq(Metadata::new(), a.clone(), c.clone());
                    Ok(Reduction::pure(Expr::And(
                        Metadata::new(),
                        Moo::new(matrix_expr![expr1, expr2]),
                    )))
                }
                _ => Err(RuleNotApplicable),
            }
        }
        _ => Err(RuleNotApplicable),
    }
}

// (A union B) subsetEq C -> A subsetEq C /\ B subsetEq C
#[register_rule(("Base", 8700))]
fn union_subseteq(expr: &Expr, _: &SymbolTable) -> ApplicationResult {
    match expr {
        Expr::SubsetEq(_, lhs, c) if matches!(c.as_ref().return_type(), ReturnType::Set(_)) => {
            match lhs.as_ref() {
                Expr::Union(_, a, b)
                    if matches!(a.as_ref().return_type(), ReturnType::Set(_))
                        && matches!(b.as_ref().return_type(), ReturnType::Set(_)) =>
                {
                    let expr1 = Expr::SubsetEq(Metadata::new(), a.clone(), b.clone());
                    let expr2 = Expr::SubsetEq(Metadata::new(), a.clone(), c.clone());
                    Ok(Reduction::pure(Expr::And(
                        Metadata::new(),
                        Moo::new(matrix_expr![expr1, expr2]),
                    )))
                }
                _ => Err(RuleNotApplicable),
            }
        }
        _ => Err(RuleNotApplicable),
    }
}
