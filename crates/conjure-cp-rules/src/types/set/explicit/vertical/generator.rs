use super::super::SetExplicit;
use conjure_cp::ast::{
    Atom, DeclarationPtr, Expression as Expr, Metadata, Moo, Reference, SymbolTable,
    comprehension::ComprehensionQualifier,
};
use conjure_cp::rule_engine::{
    ApplicationError::RuleNotApplicable, ApplicationResult, RuleEffect, register_rule,
};
use conjure_cp::{domain_int, range};
use uniplate::{Biplate, Uniplate};

fn replace_reference(expr: Expr, old_ptr: &DeclarationPtr, replacement: &Expr) -> Expr {
    expr.transform(&|candidate| match candidate {
        Expr::Atomic(_, Atom::Reference(reference)) if reference.ptr() == old_ptr => {
            replacement.clone()
        }
        other => other,
    })
}

/// Iterate explicit sets by active slots rather than testing every element-domain value.
#[register_rule("Base", 8650, [Comprehension])]
fn lower_explicit_set_expression_generator(expr: &Expr, _: &SymbolTable) -> ApplicationResult {
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
                .get_repr_as::<SetExplicit>()
                .map(|representation| (index, ptr.clone(), representation.clone()))
        })
    else {
        return Err(RuleNotApplicable);
    };

    let (_, max) = representation.cardinality;
    let index_ptr = DeclarationPtr::new_quantified(old_ptr.name().clone(), domain_int!(1..max));
    let index_expr = Expr::from(Reference::new(index_ptr.clone()));
    let value_expr = representation.slot_expr_at(index_expr.clone());
    let active = Expr::Leq(
        Metadata::new(),
        Moo::new(index_expr),
        Moo::new(representation.cardinality_expr()),
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

    #[test]
    fn explicit_set_generator_iterates_over_active_slots() {
        let parent = SymbolTablePtr::new();
        let mut set = DeclarationPtr::new_find(
            "set".into(),
            Domain::set(SetAttr::new_max_size(2), domain_int!(1..3)),
        );
        SetExplicit::init_for(&mut set).unwrap();
        parent.write().insert(set.clone());

        let mut set_reference = Reference::new(set);
        let representation = set_reference.select_repr::<SetExplicit>().unwrap().clone();
        let mut builder = ComprehensionBuilder::new(parent);
        builder = builder.expression_generator("element".into(), Expr::from(set_reference));
        let old_ptr = builder
            .generator_symboltable()
            .read()
            .lookup_local(&"element".into())
            .unwrap();
        let comprehension = builder.with_return_value(Expr::from(Reference::new(old_ptr)));
        let rewritten = lower_explicit_set_expression_generator(
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
            ComprehensionQualifier::Condition(Expr::Leq(_, index, cardinality)),
        ] = rewritten.qualifiers.as_slice()
        else {
            panic!("expected slot iteration followed by an active-slot guard");
        };
        assert_eq!(ptr.domain(), Some(domain_int!(1..2)));
        assert_eq!(index.as_ref(), &Expr::from(Reference::new(ptr.clone())));
        assert_eq!(cardinality.as_ref(), &representation.cardinality_expr());
        assert_eq!(
            rewritten.return_expression,
            representation.slot_expr_at(Expr::from(Reference::new(ptr.clone())))
        );
    }
}
