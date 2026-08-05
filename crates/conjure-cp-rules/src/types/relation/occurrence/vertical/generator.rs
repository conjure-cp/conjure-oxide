use super::super::RelationOccurrence;
use conjure_cp::ast::{
    AbstractLiteral, Atom, DeclarationPtr, Expression as Expr, Metadata, Moo, Name, Reference,
    SymbolTable, comprehension::ComprehensionQualifier,
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

/// Iterate a dense relation by its column domains directly: one domain generator per column,
/// guarded by that combination's matrix cell, yielding the tuple of column values.
#[register_rule("Base", 8650, [Comprehension])]
fn lower_relation_occurrence_expression_generator(
    expr: &Expr,
    _: &SymbolTable,
) -> ApplicationResult {
    let Expr::Comprehension(metadata, comprehension) = expr else {
        return Err(RuleNotApplicable);
    };

    // Consult the referenced declaration's own stored representation directly, rather than the
    // `Reference`'s per-instance `repr` cache: this generator source lives inside the
    // comprehension's qualifiers, which the main rewrite pass never offers individually to
    // `select_representation`, so that cache can never get populated here. Same reasoning as
    // `RelationAsSet`'s generator rule.
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
                .ptr()
                .get_repr::<RelationOccurrence>()
                .map(|representation| (index, ptr.clone(), representation.clone()))
        })
    else {
        return Err(RuleNotApplicable);
    };

    let column_ptrs: Vec<DeclarationPtr> = representation
        .inner_domains
        .iter()
        .enumerate()
        .map(|(i, dom)| {
            DeclarationPtr::new_quantified(Name::user(&format!("i{i}")), dom.clone().into())
        })
        .collect();
    let column_exprs: Vec<Expr> = column_ptrs
        .iter()
        .map(|ptr| Expr::from(Reference::new(ptr.clone())))
        .collect();
    let tuple_expr = Expr::AbstractLiteral(
        Metadata::new(),
        AbstractLiteral::Tuple(column_exprs.clone()),
    );
    let matrix_ref = Expr::from(Reference::new(representation.matrix_decl));
    let active = Expr::SafeIndex(Metadata::new(), Moo::new(matrix_ref), column_exprs);

    let mut comprehension = comprehension.as_ref().clone();
    comprehension.symbols = comprehension.symbols.detach();
    comprehension.return_expression =
        replace_reference(comprehension.return_expression, &old_ptr, &tuple_expr);
    comprehension.qualifiers = comprehension
        .qualifiers
        .into_iter()
        .enumerate()
        .map(|(i, qualifier)| {
            if i == index {
                qualifier
            } else {
                qualifier.transform_bi(&|expression: Expr| {
                    replace_reference(expression, &old_ptr, &tuple_expr)
                })
            }
        })
        .collect();

    let mut new_qualifiers: Vec<ComprehensionQualifier> = column_ptrs
        .iter()
        .cloned()
        .map(|ptr| ComprehensionQualifier::Generator { ptr })
        .collect();
    new_qualifiers.push(ComprehensionQualifier::Condition(active));
    comprehension
        .qualifiers
        .splice(index..=index, new_qualifiers);
    for ptr in &column_ptrs {
        comprehension.symbols.write().update_insert(ptr.clone());
    }

    Ok(RuleEffect::pure(Expr::Comprehension(
        metadata.clone(),
        Moo::new(comprehension),
    )))
}
