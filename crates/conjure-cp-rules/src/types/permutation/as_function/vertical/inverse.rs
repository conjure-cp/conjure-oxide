//! `PermutationAsFunction`-specific lowering of `inverse(p1, p2)`.

use super::super::PermutationAsFunction;
use conjure_cp::ast::{Atom, Expression as Expr, Metadata, Moo, Reference, SymbolTable};
use conjure_cp::rule_engine::{
    ApplicationError::RuleNotApplicable, ApplicationResult, RuleEffect, register_rule,
};

/// `inverse(p1, p2)` ("p2 is the inverse of p1") for two directly-referenced
/// `PermutationAsFunction`-represented permutations lowers to `p1.forwards = p2.backwards`, a
/// plain Function equality the Function representation's own equality rules already handle.
#[register_rule("Base", 8500, [Inverse])]
fn inverse_permutation_as_function(expr: &Expr, _: &SymbolTable) -> ApplicationResult {
    let Expr::Inverse(_, p1, p2) = expr else {
        return Err(RuleNotApplicable);
    };
    let Expr::Atomic(_, Atom::Reference(p1_ref)) = p1.as_ref() else {
        return Err(RuleNotApplicable);
    };
    let Expr::Atomic(_, Atom::Reference(p2_ref)) = p2.as_ref() else {
        return Err(RuleNotApplicable);
    };
    let Some(p1_repr) = p1_ref.ptr().get_repr::<PermutationAsFunction>() else {
        return Err(RuleNotApplicable);
    };
    let Some(p2_repr) = p2_ref.ptr().get_repr::<PermutationAsFunction>() else {
        return Err(RuleNotApplicable);
    };

    let p1_forwards = Expr::from(Reference::new(p1_repr.forwards.clone()));
    let p2_backwards = Expr::from(Reference::new(p2_repr.backwards.clone()));
    Ok(RuleEffect::pure(Expr::Eq(
        Metadata::new(),
        Moo::new(p1_forwards),
        Moo::new(p2_backwards),
    )))
}

#[cfg(test)]
mod tests {
    use super::*;
    use conjure_cp::ast::{Domain, PermutationAttr, Range};
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
    fn inverse_lowers_to_forwards_equals_backwards() {
        let p1 = permutation_decl();
        let p2 = permutation_decl();
        let p1_forwards = p1
            .get_repr::<PermutationAsFunction>()
            .unwrap()
            .forwards
            .clone();
        let p2_backwards = p2
            .get_repr::<PermutationAsFunction>()
            .unwrap()
            .backwards
            .clone();

        let expr = Expr::Inverse(
            Metadata::new(),
            Moo::new(Expr::from(Reference::new(p1))),
            Moo::new(Expr::from(Reference::new(p2))),
        );

        let rule = get_rule_by_name("inverse_permutation_as_function").expect("rule registered");
        let result = rule
            .apply(&expr, &SymbolTable::new())
            .expect("should lower inverse");

        let expected = Expr::Eq(
            Metadata::new(),
            Moo::new(Expr::from(Reference::new(p1_forwards))),
            Moo::new(Expr::from(Reference::new(p2_backwards))),
        );
        assert_eq!(result.new_expression, expected);
    }
}
