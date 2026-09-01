use super::super::MSetPacked;
use conjure_cp::ast::{
    Atom, DeclarationPtr, Expression as Expr, Metadata, Moo, Name, Reference, SymbolTable,
    comprehension::ComprehensionQualifier,
};
use conjure_cp::rule_engine::{
    ApplicationError::RuleNotApplicable, ApplicationResult, RuleEffect, register_rule,
};
use conjure_cp::{domain_int, into_matrix_expr, range};
use uniplate::{Biplate, Uniplate};

fn replace_reference(expr: Expr, old: &DeclarationPtr, replacement: &Expr) -> Expr {
    expr.transform(&|candidate| match candidate {
        Expr::Atomic(_, Atom::Reference(r)) if r.ptr() == old => replacement.clone(),
        other => other,
    })
}

/// Decode packed occurrence digits while iterating a multiset.
#[register_rule("Base", 8637, [Comprehension])]
fn lower_packed_mset_generator(expr: &Expr, _: &SymbolTable) -> ApplicationResult {
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
                    .get_repr_as::<MSetPacked>()
                    .map(|repr| (i, ptr.clone(), repr.clone()))
            })
    else {
        return Err(RuleNotApplicable);
    };
    let length = i32::try_from(repr.elements.len()).map_err(|_| RuleNotApplicable)?;
    if length == 0 || repr.occurrence.1 == 0 {
        return Err(RuleNotApplicable);
    }
    let value_index = DeclarationPtr::new_quantified(old.name().clone(), domain_int!(1..length));
    let repetition = DeclarationPtr::new_quantified(
        Name::repr(old.name().clone(), "iterator", "occurrence"),
        domain_int!(1..repr.occurrence.1),
    );
    let i = Expr::from(Reference::new(value_index.clone()));
    let values = repr
        .elements
        .iter()
        .cloned()
        .map(Expr::from)
        .collect::<Vec<_>>();
    let counts = (0..repr.elements.len())
        .map(|n| repr.element_frequency_expr(n))
        .collect::<Vec<_>>();
    let value = Expr::SafeIndex(
        Metadata::new(),
        Moo::new(into_matrix_expr!(values)),
        vec![i.clone()],
    );
    let count = Expr::SafeIndex(
        Metadata::new(),
        Moo::new(into_matrix_expr!(counts)),
        vec![i],
    );
    let active = Expr::Leq(
        Metadata::new(),
        Moo::new(Expr::from(Reference::new(repetition.clone()))),
        Moo::new(count),
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
            ComprehensionQualifier::Generator {
                ptr: value_index.clone(),
            },
            ComprehensionQualifier::Generator {
                ptr: repetition.clone(),
            },
            ComprehensionQualifier::Condition(active),
        ],
    );
    result.symbols.write().update_insert(value_index);
    result.symbols.write().update_insert(repetition);
    Ok(RuleEffect::pure(Expr::Comprehension(
        metadata.clone(),
        Moo::new(result),
    )))
}
