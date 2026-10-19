use super::super::PartitionAsSet;
use conjure_cp::ast::{Atom, Expression as Expr, Reference, SymbolTable};
use conjure_cp::rule_engine::{
    ApplicationError::RuleNotApplicable, ApplicationResult, RuleEffect, register_rule,
};

/// A standalone `parts(p)` (not a comprehension generator's source, which
/// `lower_partition_as_set_expression_generator` handles separately since a comprehension's own
/// qualifiers aren't reachable by a generically-triggered rule like this one) is, value-for-value,
/// the same set of parts as `set_decl` once `p` has [`PartitionAsSet`] selected. Needed for any
/// direct use of `parts(p)` -- e.g. `x in parts(p)`, `parts(x) = parts(y)` -- produced by this
/// type's horizontal rules.
#[register_rule("Base", 8650, [Parts])]
fn lower_partition_as_set_parts(expr: &Expr, _: &SymbolTable) -> ApplicationResult {
    let Expr::Parts(_, partition_expr) = expr else {
        return Err(RuleNotApplicable);
    };
    let Expr::Atomic(_, Atom::Reference(reference)) = partition_expr.as_ref() else {
        return Err(RuleNotApplicable);
    };
    let Some(representation) = reference.ptr().get_repr::<PartitionAsSet>() else {
        return Err(RuleNotApplicable);
    };

    Ok(RuleEffect::pure(Expr::from(Reference::new(
        representation.set_decl.clone(),
    ))))
}
