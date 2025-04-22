// Equals rule for sets
use conjure_core::ast::{Expression, SymbolTable};
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
        Eq(_, a, b) => match (a.as_ref(), b.as_ref()) {
            (Expression::AbstractLiteral(m1, a), Expression::AbstractLiteral(m2, b)) => {
                let expr1 = Expression::AbstractLiteral(m1.clone(), a.clone());
                let expr2 = Expression::AbstractLiteral(m2.clone(), b.clone());
                let expr3 = SubsetEq(
                    Metadata::new(),
                    Box::new(expr1.clone()),
                    Box::new(expr2.clone()),
                );
                let expr4 = SubsetEq(
                    Metadata::new(),
                    Box::new(expr2.clone()),
                    Box::new(expr1.clone()),
                );

                Ok(Reduction::pure(And(
                    Metadata::new(),
                    Box::new(matrix_expr![expr3.clone(), expr4.clone()]),
                )))
            }
            _ => Err(RuleNotApplicable),
        },
        _ => Err(RuleNotApplicable),
    }
}
