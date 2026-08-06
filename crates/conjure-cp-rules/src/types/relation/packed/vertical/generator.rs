use super::super::RelationPacked;
use conjure_cp::ast::{
    AbstractLiteral, Atom, DeclarationPtr, Expression as Expr, GroundDomain, HasDomain, Metadata,
    Moo, Name, Reference, SymbolTable, comprehension::ComprehensionQualifier,
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

/// Iterate a packed relation by its column domains directly: one domain generator per column,
/// guarded by that combination's occurrence bit, yielding the tuple of column values. Column
/// domains aren't stored directly on `RelationPacked` (only the enumerated tuple universe is), so
/// they're recovered from the referenced declaration's own ground domain instead.
#[register_rule("Base", 8650, [Comprehension])]
fn lower_relation_packed_expression_generator(expr: &Expr, _: &SymbolTable) -> ApplicationResult {
    let Expr::Comprehension(metadata, comprehension) = expr else {
        return Err(RuleNotApplicable);
    };

    let Some((index, old_ptr, representation, inner_doms)) = comprehension
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
            let representation = reference.ptr().get_repr::<RelationPacked>()?;
            let GroundDomain::Relation(_, inner_doms) = reference.domain_of().as_ground()?.clone()
            else {
                return None;
            };
            Some((index, ptr.clone(), representation.clone(), inner_doms))
        })
    else {
        return Err(RuleNotApplicable);
    };

    let column_ptrs: Vec<DeclarationPtr> = inner_doms
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
    let active = representation.tuple_membership_expr(&column_exprs);

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
