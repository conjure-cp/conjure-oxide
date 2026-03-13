// SupsetEq rule for sets
use conjure_cp::ast::Metadata;
use conjure_cp::ast::{Expression as Expr, ReturnType, SymbolTable, Typeable};
use conjure_cp::rule_engine::Reduction;
use conjure_cp::rule_engine::{
    ApplicationError::RuleNotApplicable, ApplicationResult, register_rule,
};

#[register_rule(("Base", 8700))]
fn supset_eq_to_subset_eq(expr: &Expr, _: &SymbolTable) -> ApplicationResult {
    match expr {
        Expr::SupsetEq(_, a, b)
            if matches!(a.as_ref().return_type(), ReturnType::Set(_))
                && matches!(b.as_ref().return_type(), ReturnType::Set(_)) =>
        {
            Ok(Reduction::pure(Expr::SubsetEq(
                Metadata::new(),
                b.clone(),
                a.clone(),
            )))
        }
        _ => Err(RuleNotApplicable),
    }
}
