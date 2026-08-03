use super::super::SetOccurrence;
use conjure_cp::ast::{
    Atom, DeclarationPtr, Expression as Expr, Metadata, Moo, Reference, SymbolTable,
    comprehension::ComprehensionQualifier,
};
use conjure_cp::rule_engine::{
    ApplicationError::RuleNotApplicable, ApplicationResult, RuleEffect, register_rule,
};
use conjure_cp::{domain_int, into_matrix_expr, range};
use uniplate::{Biplate, Uniplate};

fn replace_reference(expr: Expr, old_ptr: &DeclarationPtr, replacement: &Expr) -> Expr {
    expr.transform(&|candidate| match candidate {
        Expr::Atomic(_, Atom::Reference(reference)) if reference.ptr() == old_ptr => {
            replacement.clone()
        }
        other => other,
    })
}

/// Iterate occurrence sets by their domain values and corresponding occurrence bits.
#[register_rule("Base", 8640, [Comprehension])]
fn lower_occurrence_set_expression_generator(expr: &Expr, _: &SymbolTable) -> ApplicationResult {
    let Expr::Comprehension(metadata, comprehension) = expr else {
        return Err(RuleNotApplicable);
    };

    let Some((index, old_ptr, representation)) = comprehension
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
                .get_repr_as::<SetOccurrence>()
                .map(|representation| (index, ptr.clone(), representation.clone()))
        })
    else {
        return Err(RuleNotApplicable);
    };

    let length = i32::try_from(representation.occurs.len()).map_err(|_| RuleNotApplicable)?;
    if length == 0 {
        return Err(RuleNotApplicable);
    }
    let index_ptr = DeclarationPtr::new_quantified(old_ptr.name().clone(), domain_int!(1..length));
    let index_expr = Expr::from(Reference::new(index_ptr.clone()));
    let values = representation
        .occurs
        .iter()
        .map(|(value, _)| Expr::from(value.clone()))
        .collect::<Vec<_>>();
    let bits = representation
        .occurs
        .iter()
        .map(|(_, declaration)| Expr::from(Reference::new(declaration.clone())))
        .collect::<Vec<_>>();
    let value_expr = Expr::SafeIndex(
        Metadata::new(),
        Moo::new(into_matrix_expr!(values)),
        vec![index_expr.clone()],
    );
    let active = Expr::SafeIndex(
        Metadata::new(),
        Moo::new(into_matrix_expr!(bits)),
        vec![index_expr],
    );

    let mut comprehension = comprehension.as_ref().clone();
    comprehension.symbols = comprehension.symbols.detach();
    comprehension.return_expression =
        replace_reference(comprehension.return_expression, &old_ptr, &value_expr);
    comprehension.qualifiers = comprehension
        .qualifiers
        .into_iter()
        .map(|qualifier| {
            qualifier.transform_bi(&|expression: Expr| {
                replace_reference(expression, &old_ptr, &value_expr)
            })
        })
        .collect();
    comprehension.qualifiers.splice(
        index..=index,
        [
            ComprehensionQualifier::Generator {
                ptr: index_ptr.clone(),
            },
            ComprehensionQualifier::Condition(active),
        ],
    );
    comprehension.symbols.write().update_insert(index_ptr);

    Ok(RuleEffect::pure(Expr::Comprehension(
        metadata.clone(),
        Moo::new(comprehension),
    )))
}
