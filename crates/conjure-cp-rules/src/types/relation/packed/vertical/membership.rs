use super::super::RelationPacked;
use conjure_cp::ast::{Atom, Expression as Expr, Metadata, Moo, SymbolTable};
use conjure_cp::rule_engine::{
    ApplicationError::RuleNotApplicable, ApplicationResult, RuleEffect, register_rule,
};

/// `member in rel` on a packed relation tests the corresponding tuple's occurrence bit. `SafeIndex`
/// is valid on any tuple-shaped expression regardless of whether `member` is a literal tuple or an
/// arbitrary tuple-typed expression (e.g. a reference into another representation's own
/// declaration); `tuple_membership_expr` itself fast-paths to a direct bit test once every field
/// resolves to a constant. When `member` is a freshly-built inline tuple literal instead (as in
/// that non-constant-field case), `types::tuple::horizontal::tuple_literal_index` is what reduces
/// each `SafeIndex` built here back down to the literal field itself.
#[register_rule("Base", 8650, [In])]
fn membership_relation_packed(expr: &Expr, _: &SymbolTable) -> ApplicationResult {
    let Expr::In(_, member, collection) = expr else {
        return Err(RuleNotApplicable);
    };
    let Expr::Atomic(_, Atom::Reference(reference)) = collection.as_ref() else {
        return Err(RuleNotApplicable);
    };
    let Some(representation) = reference.ptr().get_repr::<RelationPacked>() else {
        return Err(RuleNotApplicable);
    };

    let fields: Vec<Expr> = (1..=representation.arity as i32)
        .map(|i| {
            Expr::SafeIndex(
                Metadata::new(),
                Moo::new((**member).clone()),
                vec![i.into()],
            )
        })
        .collect();

    Ok(RuleEffect::pure(
        representation.tuple_membership_expr(&fields),
    ))
}

#[cfg(test)]
mod tests {
    use super::*;
    use conjure_cp::ast::{AbstractLiteral, Domain, Reference, RelAttr};
    use conjure_cp::representation::ReprRule;
    use conjure_cp::rule_engine::get_rule_by_name;
    use conjure_cp::{domain_int, range};
    use uniplate::Uniplate;

    /// Regression: a relation column can be compound (a tuple, in this case), and `member`'s
    /// corresponding field can be a *literal* built inline (e.g. by `FunctionAsRelation`'s own
    /// forward-witness structural constraint) rather than read from a declaration. Combined with a
    /// non-constant other field (forcing `tuple_membership_expr`'s general, per-candidate branch
    /// rather than its constant-fields fast path), this used to leave `SafeIndex(member, [i])`
    /// unreduced all the way to the solver backend, which cannot index a matrix, or search a
    /// candidate list, by an un-decomposed tuple literal. Every `SafeIndex` node this rule builds
    /// must be reducible by `tuple_literal_index` (`types::tuple::horizontal`).
    #[test]
    fn membership_on_a_literal_member_with_a_compound_field_reduces_cleanly() {
        let inner_domain = Domain::tuple(vec![domain_int!(7..8), Domain::bool()]);
        let domain = Domain::relation(
            RelAttr {
                size: range!(2),
                binary: vec![],
            },
            vec![inner_domain, domain_int!(13..17)],
        );
        let mut symbols = SymbolTable::new();
        let mut declaration = symbols.gen_find(&domain);
        RelationPacked::init_for(&mut declaration).unwrap();

        let rel_ref = Expr::from(Reference::new(declaration));
        let y = symbols.gen_find(&domain_int!(13..17));
        let member = Expr::AbstractLiteral(
            Metadata::new(),
            AbstractLiteral::Tuple(vec![
                Expr::AbstractLiteral(
                    Metadata::new(),
                    AbstractLiteral::Tuple(vec![7.into(), false.into()]),
                ),
                Expr::from(Reference::new(y)),
            ]),
        );
        let expr = Expr::In(Metadata::new(), Moo::new(member), Moo::new(rel_ref));

        let membership_rule =
            get_rule_by_name("membership_relation_packed").expect("rule registered");
        let index_rule = get_rule_by_name("tuple_literal_index").expect("rule registered");

        let after_membership = membership_rule
            .apply(&expr, &symbols)
            .expect("should build a per-candidate membership check")
            .new_expression;

        let safe_indices: Vec<Expr> = after_membership
            .universe()
            .into_iter()
            .filter(|node| matches!(node, Expr::SafeIndex(..)))
            .collect();
        assert!(
            !safe_indices.is_empty(),
            "expected the membership rule to build SafeIndex nodes into `member`"
        );
        for node in &safe_indices {
            index_rule
                .apply(node, &symbols)
                .unwrap_or_else(|e| panic!("tuple_literal_index should reduce {node}, got {e:?}"));
        }
    }
}
