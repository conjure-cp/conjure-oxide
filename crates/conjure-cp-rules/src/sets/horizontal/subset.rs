// Subset rule for sets
use conjure_cp::ast::Metadata;
use conjure_cp::ast::{Expression as Expr, Moo, ReturnType, SymbolTable, Typeable};
use conjure_cp::matrix_expr;
use conjure_cp::rule_engine::Reduction;
use conjure_cp::rule_engine::{
    ApplicationError::RuleNotApplicable, ApplicationResult, register_rule,
};

#[register_rule(("Base", 8700))]
fn subset_to_subset_eq_neq(expr: &Expr, _: &SymbolTable) -> ApplicationResult {
    match expr {
        Expr::Subset(_, a, b)
            if matches!(a.as_ref().return_type(), ReturnType::Set(_))
                && matches!(b.as_ref().return_type(), ReturnType::Set(_)) =>
        {
            let expr1 = Expr::SubsetEq(Metadata::new(), a.clone(), b.clone());
            let expr2 = Expr::Neq(Metadata::new(), a.clone(), b.clone());
            Ok(Reduction::pure(Expr::And(
                Metadata::new(),
                Moo::new(matrix_expr![expr1, expr2]),
            )))
        }
        _ => Err(RuleNotApplicable),
    }
}
