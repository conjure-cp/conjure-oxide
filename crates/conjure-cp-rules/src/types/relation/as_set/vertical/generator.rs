use super::super::RelationAsSet;
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

/// A relation channelled through [`RelationAsSet`] is, value-for-value, the same set of tuples.
/// Retarget `i <- rel` onto `i <- set_decl` so that whichever set representation `set_decl` ends
/// up with (e.g. `SetExplicit`) can lower the generator the rest of the way.
#[register_rule("Base", 8650, [Comprehension])]
fn lower_relation_as_set_expression_generator(expr: &Expr, _: &SymbolTable) -> ApplicationResult {
    let Expr::Comprehension(metadata, comprehension) = expr else {
        return Err(RuleNotApplicable);
    };

    // Consult the referenced declaration's own stored representation directly, rather than the
    // `Reference`'s per-instance `repr` cache: this generator source lives inside the
    // comprehension's qualifiers, which the main rewrite pass treats as an opaque unit (it never
    // offers this specific `Reference` node to `select_representation`), so that cache can never
    // get populated here. The declaration's own repr store is updated regardless of which
    // reference path triggered it, so it is the reliable source of truth. `lower_occurrence_union_
    // expression_generator` establishes the same pattern for the same reason.
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
                let Expr::Atomic(_, Atom::Reference(reference)) = source else {
                    return None;
                };
                reference
                    .ptr()
                    .get_repr::<RelationAsSet>()
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
