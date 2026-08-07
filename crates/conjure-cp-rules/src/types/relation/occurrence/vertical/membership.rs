use super::super::RelationOccurrence;
use conjure_cp::ast::{Atom, Expression as Expr, Metadata, Moo, Reference, SymbolTable};
use conjure_cp::rule_engine::{
    ApplicationError::RuleNotApplicable, ApplicationResult, RuleEffect, register_rule,
};

/// `member in rel` on a dense relation is a direct matrix lookup, `matrix[member[1],...,member[N]]`.
/// `SafeIndex` is valid on any tuple-shaped expression regardless of whether `member` is a literal
/// tuple or an arbitrary tuple-typed expression (e.g. a reference into another representation's own
/// declaration, as `FunctionAsRelation`'s witness-based surjectivity constraint builds) -- later
/// indexing/representation rules resolve each field access independently either way. In particular,
/// when `member` is a freshly-built inline tuple literal (not yet a reference to any declaration),
/// `types::tuple::horizontal::tuple_literal_index` is what reduces each `SafeIndex` built here back
/// down to the literal field itself, rather than leaving it unresolved for the solver backend.
#[register_rule("Base", 8650, [In])]
fn membership_relation_occurrence(expr: &Expr, _: &SymbolTable) -> ApplicationResult {
    let Expr::In(_, member, collection) = expr else {
        return Err(RuleNotApplicable);
    };
    let Expr::Atomic(_, Atom::Reference(reference)) = collection.as_ref() else {
        return Err(RuleNotApplicable);
    };
    let Some(representation) = reference.ptr().get_repr::<RelationOccurrence>() else {
        return Err(RuleNotApplicable);
    };

    let arity = representation.inner_domains.len() as i32;
    let fields: Vec<Expr> = (1..=arity)
        .map(|i| {
            Expr::SafeIndex(
                Metadata::new(),
                Moo::new((**member).clone()),
                vec![i.into()],
            )
        })
        .collect();

    let matrix_ref = Expr::from(Reference::new(representation.matrix_decl.clone()));
    Ok(RuleEffect::pure(Expr::SafeIndex(
        Metadata::new(),
        Moo::new(matrix_ref),
        fields,
    )))
}

#[cfg(test)]
mod tests {
    use super::*;
    use conjure_cp::ast::{AbstractLiteral, Domain, Reference, RelAttr};
    use conjure_cp::representation::ReprRule;
    use conjure_cp::rule_engine::get_rule_by_name;
    use conjure_cp::{domain_int, range};

    /// Regression: `In` on a dense relation with a *literal* member (built inline, e.g. by
    /// `FunctionAsRelation`'s own forward-witness structural constraint, not read from a
    /// declaration) used to leave `SafeIndex(member, [i])` unreduced all the way to the solver
    /// backend, since no rule simplified indexing into a freshly-built tuple literal expression.
    /// `tuple_literal_index` (`types::tuple::horizontal`) now does, and running it to a fixpoint
    /// after this rule must leave every matrix index a plain literal.
    #[test]
    fn membership_on_a_literal_member_reduces_to_plain_literal_matrix_indices() {
        let domain = Domain::relation(
            RelAttr {
                size: range!(2),
                binary: vec![],
            },
            vec![domain_int!(1..3), domain_int!(10..12)],
        );
        let mut symbols = SymbolTable::new();
        let mut declaration = symbols.gen_find(&domain);
        RelationOccurrence::init_for(&mut declaration).unwrap();

        let rel_ref = Expr::from(Reference::new(declaration));
        let member = Expr::AbstractLiteral(
            Metadata::new(),
            AbstractLiteral::Tuple(vec![2.into(), 11.into()]),
        );
        let expr = Expr::In(Metadata::new(), Moo::new(member), Moo::new(rel_ref));

        let membership_rule =
            get_rule_by_name("membership_relation_occurrence").expect("rule registered");
        let index_rule = get_rule_by_name("tuple_literal_index").expect("rule registered");

        let after_membership = membership_rule
            .apply(&expr, &symbols)
            .expect("should lower to a matrix SafeIndex")
            .new_expression;
        let Expr::SafeIndex(_, matrix, indices) = &after_membership else {
            panic!("expected a SafeIndex into the occurrence matrix, got {after_membership}");
        };
        assert!(matches!(
            matrix.as_ref(),
            Expr::Atomic(_, Atom::Reference(_))
        ));
        assert_eq!(indices.len(), 2);

        let reduced_indices: Vec<Expr> = indices
            .iter()
            .map(|index| {
                index_rule
                    .apply(index, &symbols)
                    .expect("tuple_literal_index should reduce the literal field access")
                    .new_expression
            })
            .collect();
        assert_eq!(reduced_indices, vec![Expr::from(2), Expr::from(11)]);
    }
}
