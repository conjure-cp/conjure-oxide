// Cardinality rules for sets
use conjure_core::ast::{Atom, DeclarationKind, Domain, Expression as Expr, Literal, SymbolTable};
use conjure_core::rule_engine::{
    register_rule, ApplicationError::RuleNotApplicable, ApplicationResult,
};

use Expr::*;

use crate::ast::Declaration;
use crate::rule_engine::Rule;

#[register_rule(("Base", 8800))]
fn size_to_attr(expr: &Expr, st: &SymbolTable) -> ApplicationResult {
    if let Eq(_, a, b) = expr {
        if let (Abs(_, e), Atomic(_, d)) = (a.as_ref(), b.as_ref()) {
            if let (Atomic(_, Atom::Reference(g)), Atom::Literal(Literal::Int(_))) = (e.as_ref(), d)
            {
                if let Some(r) = st.lookup(g) {
                    if let DeclarationKind::DecisionVariable(var) = r.as_ref().kind() {
                        if let Domain::DomainSet(atr, _) = &var.domain {
                            // TODO: add cardinality depending on existing constraints
                        }
                    }
                }
            }
        }
    }
    Err(RuleNotApplicable)
}
