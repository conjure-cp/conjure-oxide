use conjure_core::into_matrix_expr;
use conjure_core::metadata::Metadata;
use conjure_core::rule_engine::Reduction;

use conjure_core::ast::Expression;

use conjure_core::ast::SymbolTable;

use conjure_core::rule_engine::{
    register_rule, ApplicationError::RuleNotApplicable, ApplicationResult,
};

use Expression::*;

#[register_rule(("Base", 8601))]
fn union_set(expr: &Expression, _: &SymbolTable) -> ApplicationResult {
    if let In(_, a, exr) = expr {
        let mut expr_ar = vec![];
        if let Union(_, c, d) = exr.as_ref() {
            expr_ar.push(Expression::In(Metadata::new(), a.clone(), c.clone()));
            expr_ar.push(Expression::In(Metadata::new(), a.clone(), d.clone()));
            return Ok(Reduction::pure(Expression::Or(
                Metadata::new(),
                Box::new(into_matrix_expr![expr_ar]),
            )));
        }
    }
    Err(RuleNotApplicable)
}

#[register_rule(("Base", 8601))]
fn inersection_set(expr: &Expression, _: &SymbolTable) -> ApplicationResult {
    if let In(_, a, exr) = expr {
        let mut expr_ar = vec![];
        if let Intersect(_, c, d) = exr.as_ref() {
            expr_ar.push(Expression::In(Metadata::new(), a.clone(), c.clone()));
            expr_ar.push(Expression::In(Metadata::new(), a.clone(), d.clone()));
            return Ok(Reduction::pure(Expression::And(
                Metadata::new(),
                Box::new(into_matrix_expr![expr_ar]),
            )));
        }
    }
    Err(RuleNotApplicable)
}
