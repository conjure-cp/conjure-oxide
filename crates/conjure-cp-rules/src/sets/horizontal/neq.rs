// Supset rule for sets
use conjure_cp::ast::Metadata;
use conjure_cp::ast::{Expression as Expr, Moo, ReturnType, SymbolTable, Typeable};
use conjure_cp::rule_engine::Reduction;
use conjure_cp::rule_engine::{
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
                        Moo::new(Expr::Eq(Metadata::new(), b.clone(), a.clone())),
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
