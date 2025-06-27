// Supset rule for sets
use conjure_core::ast::{Expression as Expr, ReturnType, SymbolTable, Typeable};
use conjure_core::metadata::Metadata;
use conjure_core::rule_engine::Reduction;
use conjure_core::rule_engine::{
    ApplicationError::RuleNotApplicable, ApplicationResult, register_rule,
};

#[register_rule(("Base", 8700))]
fn neq_not_eq_sets(expr: &Expr, _: &SymbolTable) -> ApplicationResult {
    match expr {
        Expr::Neq(_, a, b) => {
            if let Some(ReturnType::Set(_)) = a.as_ref().return_type() {
                if let Some(ReturnType::Set(_)) = b.as_ref().return_type() {
                    Ok(Reduction::pure(Expr::Not(
                        Metadata::new(),
                        Box::new(Expr::Eq(Metadata::new(), b.clone(), a.clone())),
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
