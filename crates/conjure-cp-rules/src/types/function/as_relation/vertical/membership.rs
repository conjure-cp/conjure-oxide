use super::super::FunctionAsRelation;
use super::super::representation::{exists_entry, tuple_field};
use conjure_cp::ast::{Atom, Expression as Expr, Metadata, Moo, SymbolTable};
use conjure_cp::rule_engine::{
    ApplicationError::RuleNotApplicable, ApplicationResult, RuleEffect, register_rule,
};

#[register_rule("Base", 8700, [In])]
fn membership_in_function_as_relation(expr: &Expr, _: &SymbolTable) -> ApplicationResult {
    let Expr::In(_, member, collection) = expr else {
        return Err(RuleNotApplicable);
    };
    let Expr::Defined(_, function) = collection.as_ref() else {
        return Err(RuleNotApplicable);
    };
    let Expr::Atomic(_, Atom::Reference(reference)) = function.as_ref() else {
        return Err(RuleNotApplicable);
    };
    let representation = reference
        .ptr()
        .get_repr::<FunctionAsRelation>()
        .ok_or(RuleNotApplicable)?;
    Ok(RuleEffect::pure(exists_entry(
        &representation.relation_expr(),
        |entry| {
            Expr::Eq(
                Metadata::new(),
                Moo::new(tuple_field(entry, 1)),
                member.clone(),
            )
        },
    )))
}
