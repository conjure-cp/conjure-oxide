//! `PermutationAsFunction`-specific lowering of `inverse(p1, p2)`.

use super::super::PermutationAsFunction;
use crate::types::relation::binary_attrs::quantify;
use conjure_cp::ast::ac_operators::ACOperatorKind;
use conjure_cp::ast::{Atom, Expression as Expr, Metadata, Moo, SymbolTable};
use conjure_cp::rule_engine::{
    ApplicationError::RuleNotApplicable, ApplicationResult, RuleEffect, register_rule,
};

/// `inverse(p1, p2)` ("p2 is the inverse of p1") lowers to
/// `forAll i : innerDomain . image(p2, image(p1, i)) = i` -- composing p1 then p2 back to the
/// identity everywhere is exactly what makes p2 the inverse of a bijection p1, the same one-
/// directional round-trip proof `PermutationAsFunction`'s own `structural()` uses between its
/// `forwards`/`backwards` pair. Built from `image` rather than `p1.forwards = p2.backwards`
/// (a plain Function equality) because neither `FunctionExplicit` nor `FunctionAsRelation` has an
/// `Eq` rule of their own yet -- nothing before this needed to compare two function-typed
/// *variables* -- so that shape would leave an abstract Function reference unresolved.
#[register_rule("Base", 8500, [Inverse])]
fn inverse_permutation_as_function(expr: &Expr, _: &SymbolTable) -> ApplicationResult {
    let Expr::Inverse(_, p1, p2) = expr else {
        return Err(RuleNotApplicable);
    };
    let Expr::Atomic(_, Atom::Reference(p1_ref)) = p1.as_ref() else {
        return Err(RuleNotApplicable);
    };
    let Some(p1_repr) = p1_ref.ptr().get_repr::<PermutationAsFunction>() else {
        return Err(RuleNotApplicable);
    };

    let p1 = (**p1).clone();
    let p2 = (**p2).clone();
    let body = quantify(
        std::slice::from_ref(&p1_repr.inner_domain),
        &["i"],
        ACOperatorKind::And,
        |refs| {
            let i = &refs[0];
            let p1_image = Expr::Image(Metadata::new(), Moo::new(p1.clone()), Moo::new(i.clone()));
            let round_trip = Expr::Image(Metadata::new(), Moo::new(p2.clone()), Moo::new(p1_image));
            Expr::Eq(Metadata::new(), Moo::new(round_trip), Moo::new(i.clone()))
        },
    );

    Ok(RuleEffect::pure(body))
}

#[cfg(test)]
mod tests {
    use super::*;
    use conjure_cp::ast::{Domain, PermutationAttr, Range, Reference};
    use conjure_cp::representation::ReprRule;
    use conjure_cp::rule_engine::get_rule_by_name;
    use conjure_cp::{domain_int, range};

    fn permutation_decl() -> conjure_cp::ast::DeclarationPtr {
        let domain = Domain::permutation(
            PermutationAttr::<i32> {
                num_moved: Range::Unbounded,
            },
            domain_int!(1..3),
        );
        let mut symbols = SymbolTable::new();
        let mut p = symbols.gen_find(&domain);
        <PermutationAsFunction as ReprRule>::init_for(&mut p).unwrap();
        p
    }

    #[test]
    fn inverse_lowers_to_a_pointwise_round_trip_forall() {
        let p1 = permutation_decl();
        let p2 = permutation_decl();

        let expr = Expr::Inverse(
            Metadata::new(),
            Moo::new(Expr::from(Reference::new(p1))),
            Moo::new(Expr::from(Reference::new(p2))),
        );

        let rule = get_rule_by_name("inverse_permutation_as_function").expect("registered");
        let result = rule
            .apply(&expr, &SymbolTable::new())
            .expect("should lower inverse");
        assert!(matches!(result.new_expression, Expr::And(..)));
    }

    #[test]
    fn not_applicable_when_p1_is_not_represented() {
        let p2 = permutation_decl();
        let literal = Expr::Atomic(
            Metadata::new(),
            Atom::Literal(conjure_cp::ast::Literal::AbstractLiteral(
                conjure_cp::ast::AbstractLiteral::Permutation(vec![]),
            )),
        );

        let expr = Expr::Inverse(
            Metadata::new(),
            Moo::new(literal),
            Moo::new(Expr::from(Reference::new(p2))),
        );

        let rule = get_rule_by_name("inverse_permutation_as_function").expect("registered");
        assert!(rule.apply(&expr, &SymbolTable::new()).is_err());
    }
}
