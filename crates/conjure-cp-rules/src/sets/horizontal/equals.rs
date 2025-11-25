use conjure_cp::ast::Moo;
// Equals rule for sets
use conjure_cp::ast::Metadata;
use conjure_cp::ast::{Expression, ReturnType::Set, SymbolTable, Typeable};
use conjure_cp::matrix_expr;
use conjure_cp::rule_engine::Reduction;
use conjure_cp::rule_engine::{
    ApplicationError::RuleNotApplicable, ApplicationResult, register_rule,
};

use Expression::{And, Eq, SubsetEq};

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
                        Moo::new(matrix_expr![expr1, expr2]),
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
