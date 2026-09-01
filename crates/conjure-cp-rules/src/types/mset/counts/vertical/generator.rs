use super::super::MSetCounts;
use conjure_cp::ast::{
    Atom, DeclarationPtr, Expression as Expr, Metadata, Moo, Name, Reference, SymbolTable,
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

/// Iterate each active distinct value once for every occurrence count.
#[register_rule("Base", 8637, [Comprehension])]
fn lower_counts_mset_generator(expr: &Expr, _: &SymbolTable) -> ApplicationResult {
    let Expr::Comprehension(metadata, comprehension) = expr else {
        return Err(RuleNotApplicable);
    };
    let Some((position, old, repr)) =
        comprehension
            .qualifiers
            .iter()
            .enumerate()
            .find_map(|(index, qualifier)| {
                let ComprehensionQualifier::ExpressionGenerator { ptr } = qualifier else {
                    return None;
                };
                let Expr::Atomic(_, Atom::Reference(reference)) =
                    (*ptr.as_quantified_expr()?).clone()
                else {
                    return None;
                };
                reference
                    .get_repr_as::<MSetCounts>()
                    .map(|repr| (index, ptr.clone(), repr.clone()))
            })
    else {
        return Err(RuleNotApplicable);
    };

    let value_index =
        DeclarationPtr::new_quantified(old.name().clone(), domain_int!(1..repr.max_distinct));
    let repetition = DeclarationPtr::new_quantified(
        Name::repr(old.name().clone(), "iterator", "count"),
        domain_int!(1..repr.occurrence.1),
    );
    let index_expr = Expr::from(Reference::new(value_index.clone()));
    let value = repr.value_expr_at(index_expr.clone());
    let count = repr.count_expr_at(index_expr);
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
        .map(|qualifier| {
            qualifier.transform_bi(&|expr: Expr| replace_reference(expr, &old, &value))
        })
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
