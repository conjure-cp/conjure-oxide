use std::cell::RefCell;
use std::rc::Rc;

use crate::ast::comprehension::Comprehension;
use crate::ast::comprehension::ComprehensionBuilder;
use crate::ast::comprehension::ComprehensionKind;
use crate::ast::SymbolTable;
use crate::metadata::Metadata;
use crate::rule_engine::Reduction;
use conjure_core::ast::AbstractLiteral;
use conjure_core::ast::Atom;
use conjure_core::ast::Domain;
use conjure_core::ast::Expression;
use conjure_core::ast::Literal;
use conjure_core::ast::Name;
use conjure_core::ast::Range;
use conjure_core::rule_engine::{
    register_rule, ApplicationError::RuleNotApplicable, ApplicationResult,
};
use Expression::*;

/// Converrts x in s ~~> or([ x = i | i in s ]) where s is a set (constant)
#[register_rule(("Base", 8600))]
fn in_set(expr: &Expression, st: &SymbolTable) -> ApplicationResult {
    match expr {
        In(_, a, b) => match b.as_ref() {
            AbstractLiteral(_, c) => match c {
                AbstractLiteral::Set(t) => {
                    // transform c into a domain
                    // if the domain contains decision variables then we can't apply the rule since the comprehension takes in a domain
                    let mut literals = Vec::new();
                    for expr in t {
                        if let Atomic(_, Atom::Literal(Literal::Int(i))) = expr {
                            literals.push(Range::Single(*i));
                        } else {
                            return Err(RuleNotApplicable);
                        }
                    }
                    if literals.is_empty() {
                        return Ok(Reduction::pure(Atomic(
                            Metadata::new(),
                            Atom::Literal(Literal::Bool(false)),
                        )));
                    }
                    // correct one
                    // let search_domain = Domain::IntDomain(literals);

                    // one used for testing
                    let mut min = i32::MAX;
                    let mut max = i32::MIN;
                    for number in literals.iter() {
                        if let Range::Single(value) = number {
                            if *value < min {
                                min = *value;
                            }
                            if *value > max {
                                max = *value;
                            }
                        } else {
                            return Err(RuleNotApplicable);
                        }
                    }
                    let search_domain = Domain::IntDomain(vec![Range::Bounded(min, max)]);

                    // create the comprehension builder
                    let mut builder: ComprehensionBuilder = ComprehensionBuilder::new();
                    builder = builder.generator(Name::from("i".to_string()), search_domain);

                    let ret_expr = Eq(
                        Metadata::new(),
                        a.clone(),
                        Box::new(Atomic(
                            Metadata::new(),
                            Atom::Reference(Name::from("i".to_string())),
                        )),
                    );

                    // kind is 'or' since we are looking for only one of the elements in the set
                    let comprehension: Comprehension = builder.with_return_value(
                        ret_expr,
                        Rc::new(RefCell::new(st.clone())),
                        Some(ComprehensionKind::Or),
                    );
                    let comprehension_expr: Expression =
                        Expression::Comprehension(Metadata::new(), Box::new(comprehension));

                    //  this is not correct, but is a temporary placeholder
                    Ok(Reduction::pure(Or(
                        Metadata::new(),
                        Box::new(comprehension_expr),
                    )))
                }
                _ => Err(RuleNotApplicable),
            },
            Atomic(_, Atom::Literal(Literal::AbstractLiteral(AbstractLiteral::Set(c)))) => {
                // transform a into a domain
                // if the domain contains decision variables then we can't apply the rule since the comprehension takes in a domain
                let mut literals = Vec::new();
                for number in c {
                    if let Literal::Int(n) = number {
                        literals.push(Range::Single(*n));
                    } else {
                        return Err(RuleNotApplicable);
                    }
                }
                if literals.is_empty() {
                    return Ok(Reduction::pure(Atomic(
                        Metadata::new(),
                        Atom::Literal(Literal::Bool(false)),
                    )));
                }
                // correct one
                // let search_domain = Domain::IntDomain(literals);

                // one used for testing
                let mut min = i32::MAX;
                let mut max = i32::MIN;
                for number in literals.iter() {
                    if let Range::Single(value) = number {
                        if *value < min {
                            min = *value;
                        }
                        if *value > max {
                            max = *value;
                        }
                    } else {
                        return Err(RuleNotApplicable);
                    }
                }
                let search_domain = Domain::IntDomain(vec![Range::Bounded(min, max)]);

                // create the comprehension builder
                let mut builder: ComprehensionBuilder = ComprehensionBuilder::new();
                builder = builder.generator(Name::from("i".to_string()), search_domain);

                let ret_expr = Eq(
                    Metadata::new(),
                    a.clone(),
                    Box::new(Atomic(
                        Metadata::new(),
                        Atom::Reference(Name::from("i".to_string())),
                    )),
                );

                // kind is 'or' since we are looking for only one of the elements in the set
                let comprehension: Comprehension = builder.with_return_value(
                    ret_expr,
                    Rc::new(RefCell::new(st.clone())),
                    Some(ComprehensionKind::Or),
                );
                let comprehension_expr: Expression =
                    Expression::Comprehension(Metadata::new(), Box::new(comprehension));

                //  this is not correct, but is a temporary placeholder
                Ok(Reduction::pure(Or(
                    Metadata::new(),
                    Box::new(comprehension_expr),
                )))
            }

            _ => Err(RuleNotApplicable),
        },
        _ => Err(RuleNotApplicable),
    }
}
