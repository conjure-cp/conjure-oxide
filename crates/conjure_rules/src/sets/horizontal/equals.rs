// Equals rule for sets
use conjure_core::ast::{Expression, ReturnType::Set, SymbolTable, Typeable};
use conjure_core::matrix_expr;
use conjure_core::metadata::Metadata;
use conjure_core::rule_engine::Reduction;
use conjure_core::rule_engine::{
    register_rule, ApplicationError::RuleNotApplicable, ApplicationResult,
};

use Expression::*;

#[register_rule(("Base", 8800))]
fn eq_to_subset_eq(expr: &Expression, _: &SymbolTable) -> ApplicationResult {
    match expr {
        Eq(_, a, b) => {
            if let Some(Set(_)) = a.as_ref().return_type() {
                if let Some(Set(_)) = b.as_ref().return_type() {
                    let expr1 = SubsetEq(Metadata::new(), a.clone(), b.clone());
                    let expr2 = SubsetEq(Metadata::new(), b.clone(), a.clone());
                    Ok(Reduction::pure(And(
                        Metadata::new(),
                        Box::new(matrix_expr![expr1.clone(), expr2.clone()]),
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
