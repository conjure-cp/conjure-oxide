use conjure_cp::ast::{
    Expression as Expr, Metadata, Moo, ReturnType, SymbolTable, SymbolTablePtr, Typeable,
    ac_operators::ACOperatorKind, comprehension::ComprehensionBuilder,
};
use conjure_cp::rule_engine::{
    ApplicationError::RuleNotApplicable, ApplicationResult, RuleEffect, register_rule,
};

use super::is_set_valued_index;

/// Count elements of a compound set; represented references keep their direct cardinality rules.
#[register_rule("Base", 8500, [Card])]
fn cardinality_set_expression(expr: &Expr, scope: &SymbolTable) -> ApplicationResult {
    let Expr::Card(_, source) = expr else {
        return Err(RuleNotApplicable);
    };
    if matches!(source.as_ref(), Expr::Atomic(..))
        || is_set_valued_index(source)
        || !matches!(source.return_type(), ReturnType::Set(_))
    {
        return Err(RuleNotApplicable);
    }

    let mut parent_scope = scope.clone();
    let name = parent_scope.gen_sym();
    let parent = SymbolTablePtr::new();
    *parent.write() = parent_scope;
    let builder = ComprehensionBuilder::new(parent);
    let mut comprehension = builder
        .expression_generator(name, source.as_ref().clone())
        .with_return_value(1.into());
    comprehension.skip_operator = Some(ACOperatorKind::Sum);

    Ok(RuleEffect::pure(Expr::Sum(
        Metadata::new(),
        Moo::new(Expr::Comprehension(
            Metadata::new(),
            Moo::new(comprehension),
        )),
    )))
}

#[cfg(test)]
mod tests {
    use super::*;
    use conjure_cp::ast::{
        DeclarationPtr, Domain, Name, Reference, SetAttr, comprehension::ComprehensionQualifier,
    };
    use conjure_cp::{domain_int, range};

    #[test]
    fn compound_cardinality_counts_elements_with_sum_skip_semantics() {
        let source = Expr::Intersect(
            Metadata::new(),
            Moo::new(set_reference()),
            Moo::new(set_reference()),
        );
        let cardinality = Expr::Card(Metadata::new(), Moo::new(source.clone()));
        let lowered = cardinality_set_expression(&cardinality, &SymbolTable::new())
            .unwrap()
            .new_expression;
        let Expr::Sum(_, child) = lowered else {
            panic!("expected a sum")
        };
        let Expr::Comprehension(_, comprehension) = child.as_ref() else {
            panic!("expected a comprehension")
        };
        assert_eq!(comprehension.return_expression, 1.into());
        assert_eq!(comprehension.skip_operator, Some(ACOperatorKind::Sum));
        let [ComprehensionQualifier::ExpressionGenerator { ptr }] =
            comprehension.qualifiers.as_slice()
        else {
            panic!("expected one expression generator")
        };
        assert_eq!(ptr.as_quantified_expr().as_deref(), Some(&source));
    }

    fn set_reference() -> Expr {
        Reference::new(DeclarationPtr::new_find(
            Name::user("a"),
            Domain::set(SetAttr::<i32>::default(), domain_int!(1..2)),
        ))
        .into()
    }

    #[test]
    fn represented_references_and_set_indices_keep_their_existing_lowering() {
        let reference = set_reference();
        let matrix = Reference::new(DeclarationPtr::new_find(
            Name::user("sets"),
            Domain::matrix(
                Domain::set(SetAttr::<i32>::default(), domain_int!(1..2)),
                vec![domain_int!(1..2)],
            ),
        ));
        let index = Expr::SafeIndex(Metadata::new(), Moo::new(matrix.into()), vec![1.into()]);
        for source in [reference, index] {
            assert!(matches!(
                cardinality_set_expression(
                    &Expr::Card(Metadata::new(), Moo::new(source)),
                    &SymbolTable::new()
                ),
                Err(RuleNotApplicable)
            ));
        }
    }

    #[test]
    fn counting_variable_does_not_shadow_an_outer_machine_name() {
        let mut scope = SymbolTable::new();
        let declaration =
            scope.gen_find(&Domain::set(SetAttr::<i32>::default(), domain_int!(1..2)));
        let reference = Expr::from(Reference::new(declaration.clone()));
        let source = Expr::Intersect(
            Metadata::new(),
            Moo::new(reference.clone()),
            Moo::new(reference),
        );
        let result =
            cardinality_set_expression(&Expr::Card(Metadata::new(), Moo::new(source)), &scope)
                .unwrap()
                .new_expression;
        let Expr::Sum(_, child) = result else {
            panic!("expected sum")
        };
        let Expr::Comprehension(_, comprehension) = child.as_ref() else {
            panic!("expected comprehension")
        };
        let [ComprehensionQualifier::ExpressionGenerator { ptr }] =
            comprehension.qualifiers.as_slice()
        else {
            panic!("expected generator")
        };
        let original_name = declaration.name().clone();
        assert_ne!(&*ptr.name(), &original_name);
        assert_eq!(
            comprehension.symbols.read().lookup(&original_name),
            Some(declaration)
        );
    }
}
