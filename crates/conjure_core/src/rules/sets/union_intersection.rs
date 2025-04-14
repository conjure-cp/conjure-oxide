use crate::ast::SymbolTable;
use crate::into_matrix_expr;
use crate::metadata::Metadata;
use crate::rule_engine::Reduction;
use conjure_core::ast::Expression;

use conjure_core::rule_engine::{
    register_rule, ApplicationError::RuleNotApplicable, ApplicationResult,
};

use Expression::*;

#[register_rule(("Base", 8601))]
fn union_set(expr: &Expression, st: &SymbolTable) -> ApplicationResult {
    if let In(_, a, exr) = expr {
        let mut exprAr = vec![];
        match exr.as_ref() {
            Union(_, c, d) => {
                exprAr.push(Expression::In(Metadata::new(), a.clone(), c.clone()));
                exprAr.push(Expression::In(Metadata::new(), a.clone(), d.clone()));
                return Ok(Reduction::pure(Expression::Or(
                    Metadata::new(),
                    Box::new(into_matrix_expr![exprAr]),
                )));
            }

            _ => (),
        }
    }

    return Err(RuleNotApplicable);
}
#[register_rule(("Base", 8601))]
fn inersection_set(expr: &Expression, st: &SymbolTable) -> ApplicationResult {
    if let In(_, a, exr) = expr {
        let mut exprAr = vec![];
        match exr.as_ref() {
            Intersect(_, c, d) => {
                exprAr.push(Expression::In(Metadata::new(), a.clone(), c.clone()));
                exprAr.push(Expression::In(Metadata::new(), a.clone(), d.clone()));
                return Ok(Reduction::pure(Expression::And(
                    Metadata::new(),
                    Box::new(into_matrix_expr![exprAr]),
                )));
            }

            _ => (),
        }
    }

    return Err(RuleNotApplicable);
}
