use conjure_cp::ast::{
    Atom, DeclarationPtr, Expression as Expr, Metadata, Moo, ReturnType, SymbolTable, Typeable,
    ac_operators::ACOperatorKind, comprehension::ComprehensionQualifier, eval_constant,
};
use conjure_cp::rule_engine::{
    ApplicationError::RuleNotApplicable, ApplicationResult, RuleEffect, register_rule,
};
use uniplate::Biplate;

/// Lower iteration over a set to finite-domain iteration guarded by membership.
///
/// Applies equally when the source is a compile-time constant (e.g. `i <- {1,2}`): the
/// `in`-membership condition this produces is exactly as correct there as for a decision-variable
/// source, and no other rule actually constant-folds a comprehension whose `ExpressionGenerator`
/// source is constant but whose return expression is not (every native/via-solver comprehension
/// expander explicitly declines to run at all while an `ExpressionGenerator` qualifier remains,
/// so a constant-sourced one that never reaches this rule would otherwise be stuck permanently,
/// surfacing much later as "top-level and must be flattened" once it reaches the solver adaptor
/// unexpanded -- this is exactly what `A subsetEq B` desugars into, `and([ i in B | i <- A ])`).
///
/// **Exception**: an `Or`-skip-operator comprehension (i.e. `exists i <- A . P(i)`) with a
/// constant source is deliberately left alone here, even now. `exists_quantified_to_finds`
/// (`passes/comprehensions/expansion/mod.rs`) already handles exactly that top-level shape, and
/// does it better (a domain inferred here from `A.domain_of()` can come out far looser than the
/// tight domain that rule derives, e.g. `set (unbounded) of ...` instead of the constant's actual
/// value) -- lowering it here first would pre-empt that rule and hand it a worse shape to work
/// with. Confirmed by a regression: `exists innerSet in s . x in innerSet` over a `given` set of
/// sets `s` used to solve correctly via `exists_quantified_to_finds` alone; lowering the outer
/// generator here first left a `Set`-in-`Set-of-Set` membership check that nothing else in the
/// rule set can currently discharge.
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
                if comprehension.skip_operator == Some(ACOperatorKind::Or)
                    && eval_constant(&source).is_some()
                {
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

    #[test]
    fn constant_set_generator_becomes_domain_generator_with_membership_guard() {
        use conjure_cp::ast::{AbstractLiteral, Literal};

        let constant_set = Expr::Atomic(
            Metadata::new(),
            Atom::Literal(Literal::AbstractLiteral(AbstractLiteral::Set(vec![
                Literal::Int(1),
                Literal::Int(2),
            ]))),
        );

        let mut builder = ComprehensionBuilder::new(SymbolTablePtr::new());
        builder = builder.expression_generator("element".into(), constant_set.clone());
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
        .expect("should lower a constant-sourced expression generator too")
        .new_expression;

        let Expr::Comprehension(_, rewritten) = rewritten else {
            panic!("expected a comprehension");
        };
        let [
            ComprehensionQualifier::Generator { .. },
            ComprehensionQualifier::Condition(Expr::In(_, _, source)),
        ] = rewritten.qualifiers.as_slice()
        else {
            panic!("expected a domain generator followed by membership");
        };
        assert_eq!(source.as_ref(), &constant_set);
    }

    #[test]
    fn or_skip_operator_with_a_constant_source_is_left_for_exists_quantified_to_finds() {
        // Regression test: this rule used to unconditionally lower a constant-sourced
        // ExpressionGenerator, which pre-empted `exists_quantified_to_finds`'s own, better-suited
        // handling of `exists i <- A . P(i)` at the root level (that rule infers a tighter domain
        // for `i` than `A.domain_of().element_domain()` gives here). Concretely broke
        // `exists innerSet in s . x in innerSet` over a `given` set of sets `s`.
        use conjure_cp::ast::{AbstractLiteral, Literal, ac_operators::ACOperatorKind};

        let constant_set = Expr::Atomic(
            Metadata::new(),
            Atom::Literal(Literal::AbstractLiteral(AbstractLiteral::Set(vec![
                Literal::Int(1),
                Literal::Int(2),
            ]))),
        );

        let mut builder = ComprehensionBuilder::new(SymbolTablePtr::new());
        builder = builder.expression_generator("element".into(), constant_set);
        let old_ptr = builder
            .generator_symboltable()
            .read()
            .lookup_local(&"element".into())
            .unwrap();
        let mut comprehension = builder.with_return_value(Expr::from(Reference::new(old_ptr)));
        comprehension.skip_operator = Some(ACOperatorKind::Or);

        assert!(matches!(
            lower_set_expression_generator(
                &Expr::Comprehension(Metadata::new(), Moo::new(comprehension)),
                &SymbolTable::new(),
            ),
            Err(RuleNotApplicable)
        ));
    }
}
