use conjure_cp::ast::{
    Atom, DeclarationPtr, Expression as Expr, Metadata, Moo, ReturnType, SymbolTable, Typeable,
    comprehension::ComprehensionQualifier, eval_constant,
};
use conjure_cp::rule_engine::{
    ApplicationError::RuleNotApplicable, ApplicationResult, RuleEffect, register_rule,
};
use uniplate::Biplate;

/// Lower iteration over a set to finite-domain iteration guarded by membership.
#[register_rule("Base", 8600, [Comprehension])]
fn lower_set_expression_generator(expr: &Expr, _: &SymbolTable) -> ApplicationResult {
    let Expr::Comprehension(metadata, comprehension) = expr else {
        return Err(RuleNotApplicable);
    };

    let Some((index, old_ptr, source)) =
        comprehension
            .qualifiers
            .iter()
            .enumerate()
            .find_map(|(index, qualifier)| {
                let ComprehensionQualifier::ExpressionGenerator { ptr } = qualifier else {
                    return None;
                };
                let source = (*ptr.as_quantified_expr()?).clone();
                if eval_constant(&source).is_some() {
                    return None;
                }
                matches!(source.return_type(), ReturnType::Set(_))
                    .then(|| (index, ptr.clone(), source))
            })
    else {
        return Err(RuleNotApplicable);
    };

    let element_domain = source
        .domain_of()
        .and_then(|domain| domain.element_domain())
        .ok_or(RuleNotApplicable)?;
    let replacement = DeclarationPtr::new_quantified(old_ptr.name().clone(), element_domain);
    let membership = Expr::In(
        Metadata::new(),
        Moo::new(Expr::Atomic(
            Metadata::new(),
            Atom::new_ref(replacement.clone()),
        )),
        Moo::new(source),
    );

    let mut comprehension = comprehension.as_ref().clone();
    comprehension.symbols = comprehension.symbols.detach();
    comprehension.return_expression =
        comprehension
            .return_expression
            .transform_bi(&|declaration: DeclarationPtr| {
                if declaration == old_ptr {
                    replacement.clone()
                } else {
                    declaration
                }
            });
    comprehension.qualifiers = comprehension
        .qualifiers
        .into_iter()
        .map(|qualifier| {
            qualifier.transform_bi(&|declaration: DeclarationPtr| {
                if declaration == old_ptr {
                    replacement.clone()
                } else {
                    declaration
                }
            })
        })
        .collect();
    comprehension.qualifiers.splice(
        index..=index,
        [
            ComprehensionQualifier::Generator {
                ptr: replacement.clone(),
            },
            ComprehensionQualifier::Condition(membership),
        ],
    );
    comprehension.symbols.write().update_insert(replacement);

    Ok(RuleEffect::pure(Expr::Comprehension(
        metadata.clone(),
        Moo::new(comprehension),
    )))
}

#[cfg(test)]
mod tests {
    use super::*;
    use conjure_cp::ast::{
        Domain, Reference, SetAttr, SymbolTablePtr, comprehension::ComprehensionBuilder,
    };
    use conjure_cp::{domain_int, range};

    #[test]
    fn set_generator_becomes_domain_generator_with_membership_guard() {
        let parent = SymbolTablePtr::new();
        let set = DeclarationPtr::new_find(
            "set".into(),
            Domain::set(SetAttr::new_max_size(2), domain_int!(1..2)),
        );
        parent.write().insert(set.clone());

        let mut builder = ComprehensionBuilder::new(parent);
        builder =
            builder.expression_generator("element".into(), Expr::from(Reference::new(set.clone())));
        let old_ptr = builder
            .generator_symboltable()
            .read()
            .lookup_local(&"element".into())
            .unwrap();
        let comprehension = builder.with_return_value(Expr::from(Reference::new(old_ptr)));
        let rewritten = lower_set_expression_generator(
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
            ComprehensionQualifier::Condition(Expr::In(_, member, source)),
        ] = rewritten.qualifiers.as_slice()
        else {
            panic!("expected a domain generator followed by membership");
        };
        assert_eq!(ptr.domain(), Some(domain_int!(1..2)));
        assert_eq!(member.as_ref(), &Expr::from(Reference::new(ptr.clone())));
        assert_eq!(source.as_ref(), &Expr::from(Reference::new(set)));
        assert_eq!(
            rewritten.return_expression,
            Expr::from(Reference::new(ptr.clone()))
        );
    }
}
