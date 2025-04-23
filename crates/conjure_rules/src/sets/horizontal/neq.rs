// Supset rule for sets
use conjure_core::ast::{Expression, ReturnType::Set, SymbolTable, Typeable};
use conjure_core::metadata::Metadata;
use conjure_core::rule_engine::Reduction;
use conjure_core::rule_engine::{
    register_rule, ApplicationError::RuleNotApplicable, ApplicationResult,
};

use Expression::*;

#[register_rule(("Base", 8700))]
fn neq_not_eq_sets(expr: &Expression, _: &SymbolTable) -> ApplicationResult {
    match expr {
        Neq(_, a, b) => {
            if let Some(Set(_)) = a.as_ref().return_type() {
                if let Some(Set(_)) = b.as_ref().return_type() {
                    Ok(Reduction::pure(Not(
                        Metadata::new(),
                        Box::new(Expression::Eq(Metadata::new(), b.clone(), a.clone())),
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
