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
        Eq(_, a, b)
            if matches!(a.as_ref().return_type(), Set(_))
                && matches!(b.as_ref().return_type(), Set(_)) =>
        {
            let expr1 = SubsetEq(Metadata::new(), a.clone(), b.clone());
            let expr2 = SubsetEq(Metadata::new(), b.clone(), a.clone());
            Ok(Reduction::pure(And(
                Metadata::new(),
                Moo::new(matrix_expr![expr1, expr2]),
            )))
        }
        _ => Err(RuleNotApplicable),
    }
}
