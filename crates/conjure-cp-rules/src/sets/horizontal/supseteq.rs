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
        Expr::SupsetEq(_, a, b) => {
            if let Some(ReturnType::Set(_)) = a.as_ref().return_type() {
                if let Some(ReturnType::Set(_)) = b.as_ref().return_type() {
                    Ok(Reduction::pure(Expr::SubsetEq(
                        Metadata::new(),
                        b.clone(),
                        a.clone(),
                    )))
                } else {
                    Err(RuleNotApplicable)
                }
            } else {
                Err(RuleNotApplicable)
            }
        }
        _ => Err(RuleNotApplicable),
    }
}
