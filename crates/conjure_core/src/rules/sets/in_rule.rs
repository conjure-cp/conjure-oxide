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
// TODO: change description
/// Converrts x in s ~~> or([ x = i | i in s ]) where s is a set (constant)
#[register_rule(("Base", 8600))]
fn in_set(expr: &Expression, st: &SymbolTable) -> ApplicationResult {
    match expr {
        In(_, a, b) => {
            let mut literals = Vec::new();
            let mut retur = true;

            match b.as_ref() {
                AbstractLiteral(_, c) => match c {
                    AbstractLiteral::Set(t) => {
                        for expr in t {
                            if let Atomic(_, Atom::Literal(Literal::Int(i))) = expr {
                                literals.push(*i);
                            } else {
                                retur = false;
                                break;
                            }
                        }
                    }
                    _ => retur = false,
                },
                Atomic(_, Atom::Literal(Literal::AbstractLiteral(AbstractLiteral::Set(c)))) => {
                    for number in c {
                        if let Literal::Int(n) = number {
                            literals.push(*n);
                        } else {
                            retur = false;
                        }
                    }
                }
                _ => retur = false,
            }
            if retur == false {
                return Err(RuleNotApplicable);
            }
            if literals.is_empty() {
                return Ok(Reduction::pure(Atomic(
                    Metadata::new(),
                    Atom::Literal(Literal::Bool(false)),
                )));
            }
            if let Atomic(_, a) = a.as_ref() {
                Ok(Reduction::pure(Expression::MinionWInSet(
                    Metadata::new(),
                    a.clone(),
                    literals,
                )))
            } else {
                return Err(RuleNotApplicable);
            }
        }
        _ => Err(RuleNotApplicable),
    }
}
