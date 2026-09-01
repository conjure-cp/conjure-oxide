//! `PermutationAsFunction`-specific lowering of `Eq`/`Neq` between two permutations (a directly-
//! represented permutation on at least one side; the other side can be another represented
//! permutation, a literal, or any other permutation-typed expression).
//!
//! `FunctionExplicit`/`FunctionAsRelation` have no `Eq` rule of their own (nothing before this
//! needed to compare two function-typed *variables*, only a function against a literal via other
//! paths), so comparing `forwards` directly the way `inverse`'s own vertical rule does for
//! `image(backwards, ...)` isn't available here. Decomposing to pointwise `image` equality instead
//! only relies on `image`'s own already-proven lowering (for both a represented permutation and a
//! literal one), sidestepping the gap entirely: `p = q` iff `image(p, i) = image(q, i)` for every
//! `i` in the shared inner domain.

use super::super::PermutationAsFunction;
use crate::shared::utils::as_eq_or_neq;
use crate::types::relation::binary_attrs::quantify;
use conjure_cp::ast::ac_operators::ACOperatorKind;
use conjure_cp::ast::{Atom, DomainPtr, Expression as Expr, Metadata, Moo, SymbolTable};
use conjure_cp::rule_engine::{
    ApplicationError::RuleNotApplicable, ApplicationResult, RuleEffect, register_rule,
};

/// The inner (element) domain of a `PermutationAsFunction`-represented reference, if `expr` is
/// one -- needed to quantify the pointwise `image` comparison over.
fn permutation_inner_domain(expr: &Expr) -> Option<DomainPtr> {
    let Expr::Atomic(_, Atom::Reference(reference)) = expr else {
        return None;
    };
    let representation = reference.ptr().get_repr::<PermutationAsFunction>()?;
    Some(representation.inner_domain.clone())
}

/// `p = q` (or `!=`) lowers to `forAll i : innerDomain . image(p, i) = image(q, i)` (negated as a
/// whole for `!=`, rather than existentially quantifying a per-element disequality, since that's
/// the plain logical negation and avoids a second quantify shape).
#[register_rule("Base", 8500, [Eq, Neq])]
fn permutation_as_function_eq(expr: &Expr, _: &SymbolTable) -> ApplicationResult {
    let (lhs, rhs, neq) = as_eq_or_neq(expr)?;
    let Some(inner_domain) =
        permutation_inner_domain(lhs).or_else(|| permutation_inner_domain(rhs))
    else {
        return Err(RuleNotApplicable);
    };

    let lhs = lhs.clone();
    let rhs = rhs.clone();
    let body = quantify(
        std::slice::from_ref(&inner_domain),
        &["i"],
        ACOperatorKind::And,
        |refs| {
            let i = &refs[0];
            let lhs_image =
                Expr::Image(Metadata::new(), Moo::new(lhs.clone()), Moo::new(i.clone()));
            let rhs_image =
                Expr::Image(Metadata::new(), Moo::new(rhs.clone()), Moo::new(i.clone()));
            Expr::Eq(Metadata::new(), Moo::new(lhs_image), Moo::new(rhs_image))
        },
    );

    let result = if neq {
        Expr::Not(Metadata::new(), Moo::new(body))
    } else {
        body
    };
    Ok(RuleEffect::pure(result))
}

#[cfg(test)]
mod tests {
    use super::*;
    use conjure_cp::ast::{AbstractLiteral, Domain, Literal, PermutationAttr, Range, Reference};
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
    fn eq_between_two_represented_permutations_builds_a_pointwise_forall() {
        let p = permutation_decl();
        let q = permutation_decl();

        let expr = Expr::Eq(
            Metadata::new(),
            Moo::new(Expr::from(Reference::new(p))),
            Moo::new(Expr::from(Reference::new(q))),
        );

        let rule = get_rule_by_name("permutation_as_function_eq").expect("registered");
        let result = rule
            .apply(&expr, &SymbolTable::new())
            .expect("should lower");
        assert!(matches!(result.new_expression, Expr::And(..)));
    }

    #[test]
    fn neq_wraps_the_forall_in_not() {
        let p = permutation_decl();
        let q = permutation_decl();

        let expr = Expr::Neq(
            Metadata::new(),
            Moo::new(Expr::from(Reference::new(p))),
            Moo::new(Expr::from(Reference::new(q))),
        );

        let rule = get_rule_by_name("permutation_as_function_eq").expect("registered");
        let result = rule
            .apply(&expr, &SymbolTable::new())
            .expect("should lower");
        assert!(matches!(result.new_expression, Expr::Not(..)));
    }

    #[test]
    fn eq_against_a_literal_still_fires_off_the_represented_side() {
        let p = permutation_decl();
        let literal = Expr::Atomic(
            Metadata::new(),
            Atom::Literal(Literal::AbstractLiteral(AbstractLiteral::Permutation(
                vec![vec![1.into(), 2.into()]],
            ))),
        );

        let expr = Expr::Eq(
            Metadata::new(),
            Moo::new(Expr::from(Reference::new(p))),
            Moo::new(literal),
        );

        let rule = get_rule_by_name("permutation_as_function_eq").expect("registered");
        assert!(rule.apply(&expr, &SymbolTable::new()).is_ok());
    }

    #[test]
    fn not_applicable_when_neither_side_is_represented() {
        let literal_a = Expr::Atomic(
            Metadata::new(),
            Atom::Literal(Literal::AbstractLiteral(AbstractLiteral::Permutation(
                vec![],
            ))),
        );
        let literal_b = literal_a.clone();
        let expr = Expr::Eq(Metadata::new(), Moo::new(literal_a), Moo::new(literal_b));

        let rule = get_rule_by_name("permutation_as_function_eq").expect("registered");
        assert!(rule.apply(&expr, &SymbolTable::new()).is_err());
    }
}
