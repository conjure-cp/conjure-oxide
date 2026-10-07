use conjure_cp::ast::SymbolTable;
use conjure_cp::ast::{Atom, Expression as Expr};
use conjure_cp::rule_engine::{
    ApplicationError::RuleNotApplicable, ApplicationResult, RuleEffect, register_rule,
};

use crate::types::int::IntOrder;

/// Replace an order-represented reference with its bit vector.
#[register_rule("SAT", 9500, [Atomic])]
fn integer_decision_representation_order(expr: &Expr, _: &SymbolTable) -> ApplicationResult {
    let Expr::Atomic(_, Atom::Reference(reference)) = expr else {
        return Err(RuleNotApplicable);
    };
    let state = reference
        .get_repr_as::<IntOrder>()
        .ok_or(RuleNotApplicable)?;
    Ok(RuleEffect::pure(state.sat_int_expr()))
}
