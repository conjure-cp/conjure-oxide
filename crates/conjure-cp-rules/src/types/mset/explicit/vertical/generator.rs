use super::super::MSetExplicit;
use conjure_cp::ast::{
    Atom, DeclarationPtr, Expression as Expr, Metadata, Moo, Reference, SymbolTable,
    comprehension::ComprehensionQualifier,
};
use conjure_cp::rule_engine::{
    ApplicationError::RuleNotApplicable, ApplicationResult, RuleEffect, register_rule,
};
use conjure_cp::{domain_int, range};
use uniplate::{Biplate, Uniplate};

fn replace_reference(expr: Expr, old: &DeclarationPtr, replacement: &Expr) -> Expr {
    expr.transform(&|candidate| match candidate {
        Expr::Atomic(_, Atom::Reference(reference)) if reference.ptr() == old => {
            replacement.clone()
        }
        other => other,
    })
}

/// Iterate explicit multisets by active slots, retaining repeated values.
#[register_rule("Base", 8639, [Comprehension])]
fn lower_explicit_mset_generator(expr: &Expr, _: &SymbolTable) -> ApplicationResult {
    let Expr::Comprehension(metadata, comprehension) = expr else {
        return Err(RuleNotApplicable);
    };
    let Some((position, old, repr)) =
        comprehension
            .qualifiers
            .iter()
            .enumerate()
            .find_map(|(i, q)| {
                let ComprehensionQualifier::ExpressionGenerator { ptr } = q else {
                    return None;
                };
                let Expr::Atomic(_, Atom::Reference(reference)) =
                    (*ptr.as_quantified_expr()?).clone()
                else {
                    return None;
                };
                reference
                    .get_repr_as::<MSetExplicit>()
                    .map(|repr| (i, ptr.clone(), repr.clone()))
            })
    else {
        return Err(RuleNotApplicable);
    };

    let max = repr.cardinality.1;
    let index = DeclarationPtr::new_quantified(old.name().clone(), domain_int!(1..max));
    let index_expr = Expr::from(Reference::new(index.clone()));
    let value = repr.slot_expr_at(index_expr.clone());
    let active = Expr::Leq(
        Metadata::new(),
        Moo::new(index_expr),
        Moo::new(repr.cardinality_expr()),
    );
    let mut result = comprehension.as_ref().clone();
    result.symbols = result.symbols.detach();
    result.return_expression = replace_reference(result.return_expression, &old, &value);
    result.qualifiers = result
        .qualifiers
        .into_iter()
        .map(|q| q.transform_bi(&|e: Expr| replace_reference(e, &old, &value)))
        .collect();
    result.qualifiers.splice(
        position..=position,
        [
            ComprehensionQualifier::Generator { ptr: index.clone() },
            ComprehensionQualifier::Condition(active),
        ],
    );
    result.symbols.write().update_insert(index);
    Ok(RuleEffect::pure(Expr::Comprehension(
        metadata.clone(),
        Moo::new(result),
    )))
}
