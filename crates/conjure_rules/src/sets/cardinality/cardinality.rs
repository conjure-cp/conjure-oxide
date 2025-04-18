// Cardinality rules for sets
use conjure_core::ast::{Atom, DeclarationKind, Domain, Expression, Literal, SymbolTable};
use conjure_core::metadata::Metadata;
use conjure_core::rule_engine::{
    register_rule, ApplicationError::RuleNotApplicable, ApplicationResult,
};

use std::rc::Rc;
use Expression::*;

use crate::ast::{Declaration, SetAttr};
use crate::rule_engine::Reduction;

// rule that loads the cardinality of a set into the attribute of a set
// this rule is only applicable if the set is a decision variable and the cardinality is not already loaded
#[register_rule(("Base", 8901))]
fn card_to_attr(expr: &Expression, st: &SymbolTable) -> ApplicationResult {
    let mut code = 0;
    let (a, b) = match expr {
        Eq(_, a, b) => {
            code = 1;
            (Some(a), Some(b))
        }
        Leq(_, a, b) => {
            code = 2;
            (Some(a), Some(b))
        }
        Geq(_, a, b) => {
            code = 3;
            (Some(a), Some(b))
        }
        Lt(_, a, b) => {
            code = 4;
            (Some(a), Some(b))
        }
        Gt(_, a, b) => {
            code = 5;
            (Some(a), Some(b))
        }
        _ => (None, None),
    };
    if let (Some(a), Some(b)) = (a, b) {
        if let (Abs(_, e), Atomic(_, d)) = (a.as_ref(), b.as_ref()) {
            // d can't be a variable since the cardinality will be treated differently depending on the representation of the set
            if let (Atomic(_, Atom::Reference(g)), Atom::Literal(Literal::Int(size))) =
                (e.as_ref(), d)
            {
                if let Some(r) = st.lookup(g) {
                    if let DeclarationKind::DecisionVariable(var) = r.kind() {
                        if let Domain::DomainSet(atr, vals) = &var.domain {
                            let mut symbols = st.clone();
                            let mut new_domain = Domain::DomainSet(atr.clone(), vals.clone());
                            match atr {
                                SetAttr::Size(s) => {
                                    match code {
                                        1 => {
                                            if s == size {
                                                return Err(RuleNotApplicable);
                                            } else {
                                                return Ok(Reduction::pure(Expression::Atomic(
                                                    Metadata::new(),
                                                    Atom::Literal(Literal::Bool(false)),
                                                )));
                                            }
                                        }

                                        2 => {
                                            if s <= size {
                                                return Err(RuleNotApplicable);
                                            } else {
                                                return Ok(Reduction::pure(Expression::Atomic(
                                                    Metadata::new(),
                                                    Atom::Literal(Literal::Bool(false)),
                                                )));
                                            }
                                        }
                                        3 => {
                                            if s >= size {
                                                return Err(RuleNotApplicable);
                                            } else {
                                                return Ok(Reduction::pure(Expression::Atomic(
                                                    Metadata::new(),
                                                    Atom::Literal(Literal::Bool(false)),
                                                )));
                                            }
                                        }
                                        4 => {
                                            if s < size {
                                                return Err(RuleNotApplicable);
                                            } else {
                                                return Ok(Reduction::pure(Expression::Atomic(
                                                    Metadata::new(),
                                                    Atom::Literal(Literal::Bool(false)),
                                                )));
                                            }
                                        }
                                        5 => {
                                            if s > size {
                                                return Err(RuleNotApplicable);
                                            } else {
                                                return Ok(Reduction::pure(Expression::Atomic(
                                                    Metadata::new(),
                                                    Atom::Literal(Literal::Bool(false)),
                                                )));
                                            }
                                        }
                                        _ => return Err(RuleNotApplicable),
                                    };
                                }
                                SetAttr::None => {
                                    new_domain = match code {
                                        1 => Domain::DomainSet(SetAttr::Size(*size), vals.clone()),
                                        2 => {
                                            Domain::DomainSet(SetAttr::MaxSize(*size), vals.clone())
                                        }
                                        3 => {
                                            Domain::DomainSet(SetAttr::MinSize(*size), vals.clone())
                                        }
                                        4 => Domain::DomainSet(
                                            SetAttr::MaxSize(*size - 1),
                                            vals.clone(),
                                        ),
                                        5 => Domain::DomainSet(
                                            SetAttr::MinSize(*size + 1),
                                            vals.clone(),
                                        ),
                                        _ => {
                                            println!("Case not covered!");
                                            return Err(RuleNotApplicable);
                                        }
                                    };
                                }
                                // TODO: add other cases in the future if needed. Not that important for now.
                                _ => {
                                    println!("Case not covered!");
                                    Domain::DomainSet(SetAttr::None, vals.clone());
                                }
                            }
                            symbols.update_insert(Rc::new(Declaration::new_var(
                                g.clone(),
                                new_domain,
                            )));
                            println!("{:?}", symbols.lookup(g));
                            return Ok(Reduction::with_symbols(
                                Expression::Atomic(
                                    Metadata::new(),
                                    Atom::Literal(Literal::Bool(true)),
                                ),
                                symbols,
                            ));
                        }
                    }
                }
            }
        }
    }

    Err(RuleNotApplicable)
}
