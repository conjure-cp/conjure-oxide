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
// TODO: comment explanations

// TODO: add cardinality depending on existing constraints if needed??
// TODO: check special cases like : 0 < |A| < 2 => |A| = 1 and other exceptions if needed??
// TODO: should we cover the case : |A| != a (Neq) ??
#[register_rule(("Base", 8900))]
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
        // Neq(_, a, b) => {
        //     code = 6;
        //     (Some(a), Some(b))
        // }
        _ => (None, None),
    };
    if let Some(a) = a {
        if let Some(b) = b {
            if let (Abs(_, e), Atomic(_, d)) = (a.as_ref(), b.as_ref()) {
                if let (Atomic(_, Atom::Reference(g)), Atom::Literal(Literal::Int(size))) =
                    (e.as_ref(), d)
                {
                    if let Some(r) = st.lookup(g) {
                        if let DeclarationKind::DecisionVariable(var) = r.kind() {
                            if let Domain::DomainSet(atr, vals) = &var.domain {
                                let mut symbols = st.clone();
                                match atr {
                                    SetAttr::None => {
                                        let new_domain = match code {
                                            1 => Domain::DomainSet(
                                                SetAttr::Size(*size),
                                                vals.clone(),
                                            ),
                                            2 => Domain::DomainSet(
                                                SetAttr::MaxSize(*size),
                                                vals.clone(),
                                            ),
                                            3 => Domain::DomainSet(
                                                SetAttr::MinSize(*size),
                                                vals.clone(),
                                            ),
                                            4 => Domain::DomainSet(
                                                SetAttr::MaxSize(*size - 1),
                                                vals.clone(),
                                            ),
                                            5 => Domain::DomainSet(
                                                SetAttr::MinSize(*size + 1),
                                                vals.clone(),
                                            ),
                                            _ => Domain::DomainSet(SetAttr::None, vals.clone()),
                                        };

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
                                    _ => todo!(),
                                }
                            }
                        }
                    }
                }
            }
        }
    }

    Err(RuleNotApplicable)
}
