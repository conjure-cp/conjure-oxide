use super::super::PartitionAsSet;
use conjure_cp::ast::{
    Atom, DeclarationPtr, Expression as Expr, Moo, Reference, SymbolTable,
    comprehension::ComprehensionQualifier,
};
use conjure_cp::rule_engine::{
    ApplicationError::RuleNotApplicable, ApplicationResult, RuleEffect, register_rule,
};
use uniplate::{Biplate, Uniplate};

fn replace_reference(expr: Expr, old_ptr: &DeclarationPtr, replacement: &Expr) -> Expr {
    expr.transform(&|candidate| match candidate {
        Expr::Atomic(_, Atom::Reference(reference)) if reference.ptr() == old_ptr => {
            replacement.clone()
        }
        other => other,
    })
}

/// A partition channelled through [`PartitionAsSet`] is, value-for-value, the same set of parts.
/// Retarget `i <- parts(p)` onto `i <- set_decl` so that whichever set representation `set_decl`
/// ends up with can lower the generator the rest of the way. Mirrors Conjure's
/// `partition-comprehension{PartitionAsSet}`.
#[register_rule("Base", 8650, [Comprehension])]
fn lower_partition_as_set_expression_generator(expr: &Expr, _: &SymbolTable) -> ApplicationResult {
    let Expr::Comprehension(metadata, comprehension) = expr else {
        return Err(RuleNotApplicable);
    };

    // See `lower_relation_as_set_expression_generator`'s comment for why the declaration's own
    // repr store, not the `Reference`'s per-instance cache, is the reliable source of truth here.
    let Some((index, old_ptr, set_decl)) =
        comprehension
            .qualifiers
            .iter()
            .enumerate()
            .find_map(|(index, qualifier)| {
                let ComprehensionQualifier::ExpressionGenerator { ptr } = qualifier else {
                    return None;
                };
                let source = (*ptr.as_quantified_expr()?).clone();
                let Expr::Parts(_, partition_expr) = source else {
                    return None;
                };
                let Expr::Atomic(_, Atom::Reference(reference)) = partition_expr.as_ref() else {
                    return None;
                };
                reference
                    .ptr()
                    .get_repr::<PartitionAsSet>()
                    .map(|representation| (index, ptr.clone(), representation.set_decl.clone()))
            })
    else {
        return Err(RuleNotApplicable);
    };

    let new_ptr = DeclarationPtr::new_quantified_expr(
        old_ptr.name().clone(),
        Expr::from(Reference::new(set_decl)),
    );
    let new_ref_expr = Expr::from(Reference::new(new_ptr.clone()));

    let mut comprehension = comprehension.as_ref().clone();
    comprehension.symbols = comprehension.symbols.detach();
    comprehension.return_expression =
        replace_reference(comprehension.return_expression, &old_ptr, &new_ref_expr);
    comprehension.qualifiers = comprehension
        .qualifiers
        .into_iter()
        .enumerate()
        .map(|(i, qualifier)| {
            if i == index {
                ComprehensionQualifier::ExpressionGenerator {
                    ptr: new_ptr.clone(),
                }
            } else {
                qualifier.transform_bi(&|expression: Expr| {
                    replace_reference(expression, &old_ptr, &new_ref_expr)
                })
            }
        })
        .collect();
    comprehension.symbols.write().update_insert(new_ptr);

    Ok(RuleEffect::pure(Expr::Comprehension(
        metadata.clone(),
        Moo::new(comprehension),
    )))
}
