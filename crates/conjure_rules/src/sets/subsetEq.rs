use std::cell::RefCell;
use std::rc::Rc;

use conjure_core::ast::comprehension::ComprehensionBuilder;
use conjure_core::ast::comprehension::ComprehensionKind;
use conjure_core::ast::Domain;
use conjure_core::ast::Name;
use conjure_core::metadata::Metadata;
use conjure_core::rule_engine::Reduction;

use conjure_core::ast::AbstractLiteral;
use conjure_core::ast::Atom;
use conjure_core::ast::Expression;
use conjure_core::ast::Literal;

use conjure_core::ast::SymbolTable;

use conjure_core::rule_engine::{
    register_rule, ApplicationError::RuleNotApplicable, ApplicationResult,
};

use Expression::*;

#[register_rule(("Base", 8600))]
fn subset_eq(expr: &Expression, st: &SymbolTable) -> ApplicationResult {
    if let SubSetEq(_, a, b) = expr {
        println!("qwertqweqweqeqweqweqweqewqewy");

        let mut builder: ComprehensionBuilder = ComprehensionBuilder::new();
        builder = builder.special_generator(Name::from("i".to_string()), *a.clone());
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

    return Err(RuleNotApplicable);
}
