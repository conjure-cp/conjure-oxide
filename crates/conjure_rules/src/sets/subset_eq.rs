use std::cell::RefCell;
use std::rc::Rc;

use conjure_core::ast::comprehension::ComprehensionBuilder;
use conjure_core::ast::comprehension::ComprehensionKind;
use conjure_core::ast::comprehension::Generator;
use conjure_core::ast::Name;
use conjure_core::into_matrix_expr;
use conjure_core::metadata::Metadata;
use conjure_core::rule_engine::Reduction;

use conjure_core::ast::Atom;
use conjure_core::ast::Expression;

use conjure_core::ast::SymbolTable;

use conjure_core::rule_engine::{
    register_rule, ApplicationError::RuleNotApplicable, ApplicationResult,
};

use Expression::*;

#[register_rule(("Base", 8600))]
fn subset_eq(expr: &Expression, st: &SymbolTable) -> ApplicationResult {
    if let SubSetEq(_, a, b) = expr {
        let mut builder: ComprehensionBuilder = ComprehensionBuilder::new();
        builder = builder.generator(Name::from("i".to_string()), Generator::InExpr(*a.clone()));
        let ret_expr = In(
            Metadata::new(),
            Box::new(Atomic(
                Metadata::new(),
                Atom::Reference(Name::from("i".to_string())),
            )),
            b.clone(),
        );
        let comprehension = builder.with_return_value(
            ret_expr,
            Rc::new(RefCell::new(st.clone())),
            Some(ComprehensionKind::And),
        );
        let comprehension_expr: Expression =
            Expression::Comprehension(Metadata::new(), Box::new(comprehension));
        return Ok(Reduction::pure(And(
            Metadata::new(),
            Box::new(comprehension_expr),
        )));
    }
    Err(RuleNotApplicable)
}

#[register_rule(("Base", 8601))]
fn split_union_subseteq(expr: &Expression, _: &SymbolTable) -> ApplicationResult {
    if let SubSetEq(_, a, b) = expr {
        if let Union(_, c, d) = a.as_ref() {
            let mut expr_ar = vec![Expression::SubSetEq(Metadata::new(), c.clone(), b.clone())];
            expr_ar.push(Expression::SubSetEq(Metadata::new(), d.clone(), b.clone()));
            return Ok(Reduction::pure(Expression::And(
                Metadata::new(),
                Box::new(into_matrix_expr![expr_ar]),
            )));
        }
    }
    Err(RuleNotApplicable)
}

#[register_rule(("Base", 8601))]
fn split_intersection_subseteq(expr: &Expression, st: &SymbolTable) -> ApplicationResult {
    if let SubSetEq(_, a, b) = expr {
        if let Intersect(_, c, d) = a.as_ref() {
            let mut builder: ComprehensionBuilder = ComprehensionBuilder::new();
            builder = builder.generator(Name::from("i".to_string()), Generator::InExpr(*d.clone()));
            builder = builder.guard(Expression::In(
                Metadata::new(),
                Box::new(Atomic(
                    Metadata::new(),
                    Atom::Reference(Name::from("i".to_string())),
                )),
                c.clone(),
            ));
            let ret_expr = In(
                Metadata::new(),
                Box::new(Atomic(
                    Metadata::new(),
                    Atom::Reference(Name::from("i".to_string())),
                )),
                b.clone(),
            );
            let comprehension = builder.with_return_value(
                ret_expr,
                Rc::new(RefCell::new(st.clone())),
                Some(ComprehensionKind::And),
            );
            let comprehension_expr: Expression =
                Expression::Comprehension(Metadata::new(), Box::new(comprehension));
            return Ok(Reduction::pure(And(
                Metadata::new(),
                Box::new(comprehension_expr),
            )));
        }
    }
    Err(RuleNotApplicable)
}
