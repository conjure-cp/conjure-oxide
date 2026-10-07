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

/// Iterate a union of occurrence sets once over its element domain, guarded by membership in
/// either operand. This avoids the horizontal union rule's duplicate-free branch split and its
/// nested `flatten` expressions.
#[register_rule("ReprGeneral", 8750, [Comprehension])]
fn lower_occurrence_union_expression_generator(expr: &Expr, _: &SymbolTable) -> ApplicationResult {
    let Expr::Comprehension(metadata, comprehension) = expr else {
        return Err(RuleNotApplicable);
    };

    let Some((index, old_ptr, lhs, rhs, element_domain)) = comprehension
        .qualifiers
        .iter()
        .enumerate()
        .find_map(|(index, qualifier)| {
            let ComprehensionQualifier::ExpressionGenerator { ptr } = qualifier else {
                return None;
            };
            let source = (*ptr.as_quantified_expr()?).clone();
            let Expr::Union(_, lhs, rhs) = &source else {
                return None;
            };
            let (
                Expr::Atomic(_, Atom::Reference(lhs_reference)),
                Expr::Atomic(_, Atom::Reference(rhs_reference)),
            ) = (lhs.as_ref(), rhs.as_ref())
            else {
                return None;
            };
            let _ = lhs_reference.ptr().get_repr::<SetOccurrence>()?;
            let _ = rhs_reference.ptr().get_repr::<SetOccurrence>()?;
            let element_domain = source.domain_of()?.element_domain()?;
            Some((
                index,
                ptr.clone(),
                lhs.as_ref().clone(),
                rhs.as_ref().clone(),
                element_domain,
            ))
        })
    else {
        return Err(RuleNotApplicable);
    };

    let value_ptr = DeclarationPtr::new_quantified(old_ptr.name().clone(), element_domain);
    let value = Expr::from(Reference::new(value_ptr.clone()));
    let active = Expr::Or(
        Metadata::new(),
        Moo::new(into_matrix_expr!(vec![
            Expr::In(Metadata::new(), Moo::new(value.clone()), Moo::new(lhs)),
            Expr::In(Metadata::new(), Moo::new(value.clone()), Moo::new(rhs)),
        ])),
    );

    let mut comprehension = comprehension.as_ref().clone();
    comprehension.symbols = comprehension.symbols.detach();
    comprehension.return_expression =
        replace_reference(comprehension.return_expression, &old_ptr, &value);
    comprehension.qualifiers = comprehension
        .qualifiers
        .into_iter()
        .map(|qualifier| {
            qualifier
                .transform_bi(&|expression: Expr| replace_reference(expression, &old_ptr, &value))
        })
        .collect();
    comprehension.qualifiers.splice(
        index..=index,
        [
            ComprehensionQualifier::Generator {
                ptr: value_ptr.clone(),
            },
            ComprehensionQualifier::Condition(active),
        ],
    );
    comprehension.symbols.write().update_insert(value_ptr);

    Ok(RuleEffect::pure(Expr::Comprehension(
        metadata.clone(),
        Moo::new(comprehension),
    )))
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

#[cfg(test)]
mod tests {
    use super::*;
    use conjure_cp::ast::{Domain, SetAttr, SymbolTablePtr, comprehension::ComprehensionBuilder};
    use conjure_cp::representation::ReprRule;
    use conjure_cp::{domain_int, range};

    #[test]
    fn occurrence_union_generator_uses_one_domain_generator_with_membership_guard() {
        let parent = SymbolTablePtr::new();
        let domain = Domain::set(SetAttr::<i32>::default(), domain_int!(1..3));
        let mut lhs = DeclarationPtr::new_find("lhs".into(), domain.clone());
        let mut rhs = DeclarationPtr::new_find("rhs".into(), domain);
        SetOccurrence::init_for(&mut lhs).unwrap();
        SetOccurrence::init_for(&mut rhs).unwrap();
        parent.write().insert(lhs.clone());
        parent.write().insert(rhs.clone());

        let union = Expr::Union(
            Metadata::new(),
            Moo::new(Expr::from(Reference::new(lhs.clone()))),
            Moo::new(Expr::from(Reference::new(rhs.clone()))),
        );
        let mut builder = ComprehensionBuilder::new(parent);
        builder = builder.expression_generator("element".into(), union);
        let old_ptr = builder
            .generator_symboltable()
            .read()
            .lookup_local(&"element".into())
            .unwrap();
        let comprehension = builder.with_return_value(Expr::from(Reference::new(old_ptr)));

        let rewritten = lower_occurrence_union_expression_generator(
            &Expr::Comprehension(Metadata::new(), Moo::new(comprehension)),
            &SymbolTable::new(),
        )
        .unwrap()
        .new_expression;

        let Expr::Comprehension(_, rewritten) = rewritten else {
            panic!("expected a comprehension");
        };
        let [
            ComprehensionQualifier::Generator { ptr },
            ComprehensionQualifier::Condition(Expr::Or(_, guards)),
        ] = rewritten.qualifiers.as_slice()
        else {
            panic!("expected one domain generator and one disjunctive membership guard");
        };
        assert_eq!(ptr.domain(), Some(domain_int!(1..3)));
        assert!(
            Moo::unwrap_or_clone(guards.clone())
                .unwrap_list()
                .unwrap()
                .iter()
                .all(|guard| matches!(guard, Expr::In(..)))
        );
        assert_eq!(
            rewritten.return_expression,
            Expr::from(Reference::new(ptr.clone()))
        );
    }
}
