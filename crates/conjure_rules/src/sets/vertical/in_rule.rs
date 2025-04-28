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
fn in_set(expr: &Expression, _: &SymbolTable) -> ApplicationResult {
    match expr {
        In(_, a, b) => {
            let mut literals = Vec::new();
            let mut retur = true;

            // TODO: add case where a is a variable
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

                _ => return Err(RuleNotApplicable),
            }
            if retur {
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
                    Err(RuleNotApplicable)
                }
            } else {
                Err(RuleNotApplicable)
            }
        }
        _ => Err(RuleNotApplicable),
    }
}
