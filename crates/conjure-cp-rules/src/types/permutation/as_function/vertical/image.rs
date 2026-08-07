//! `PermutationAsFunction`-specific lowering of `image`/`permInverse`, mirroring Conjure's
//! `Rules/Vertical/Permutation/PermutationAsFunction.hs`.
//!
//! Both rules just redirect to the chosen representation's `forwards`/`backwards` Function-typed
//! aux declarations -- lowering `image` on *those* is the Function representation's own job, not
//! this one's.

use super::super::PermutationAsFunction;
use conjure_cp::ast::{Atom, Expression as Expr, Metadata, Moo, Reference, SymbolTable};
use conjure_cp::rule_engine::{
    ApplicationError::RuleNotApplicable, ApplicationResult, RuleEffect, register_rule,
};

/// `image(p, i)` for a directly-referenced `PermutationAsFunction`-represented permutation
/// lowers to `image(forwards, i)`.
#[register_rule("Base", 8500, [Image])]
fn image_permutation_as_function(expr: &Expr, _: &SymbolTable) -> ApplicationResult {
    let Expr::Image(_, function, arg) = expr else {
        return Err(RuleNotApplicable);
    };
    let Expr::Atomic(_, Atom::Reference(reference)) = function.as_ref() else {
        return Err(RuleNotApplicable);
    };
    let Some(representation) = reference.ptr().get_repr::<PermutationAsFunction>() else {
        return Err(RuleNotApplicable);
    };

    let forwards_ref = Expr::from(Reference::new(representation.forwards.clone()));
    Ok(RuleEffect::pure(Expr::Image(
        Metadata::new(),
        Moo::new(forwards_ref),
        arg.clone(),
    )))
}

/// `image(permInverse(p), i)` for a directly-referenced `PermutationAsFunction`-represented
/// permutation lowers to `image(backwards, i)` -- the same redirect as above, but through the
/// backwards mapping instead of the forwards one.
#[register_rule("Base", 8501, [Image])]
fn image_perm_inverse_permutation_as_function(expr: &Expr, _: &SymbolTable) -> ApplicationResult {
    let Expr::Image(_, function, arg) = expr else {
        return Err(RuleNotApplicable);
    };
    let Expr::PermInverse(_, inner) = function.as_ref() else {
        return Err(RuleNotApplicable);
    };
    let Expr::Atomic(_, Atom::Reference(reference)) = inner.as_ref() else {
        return Err(RuleNotApplicable);
    };
    let Some(representation) = reference.ptr().get_repr::<PermutationAsFunction>() else {
        return Err(RuleNotApplicable);
    };

    let backwards_ref = Expr::from(Reference::new(representation.backwards.clone()));
    Ok(RuleEffect::pure(Expr::Image(
        Metadata::new(),
        Moo::new(backwards_ref),
        arg.clone(),
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
    fn image_redirects_to_the_forwards_declaration() {
        let p = permutation_decl();
        let representation = p.get_repr::<PermutationAsFunction>().unwrap();
        let forwards_decl = representation.forwards.clone();
        drop(representation);

        let p_ref = Expr::from(Reference::new(p.clone()));
        let arg = Expr::Atomic(Metadata::new(), Atom::Literal(1.into()));
        let expr = Expr::Image(Metadata::new(), Moo::new(p_ref), Moo::new(arg.clone()));

        let rule = get_rule_by_name("image_permutation_as_function").expect("rule registered");
        let result = rule
            .apply(&expr, &SymbolTable::new())
            .expect("should redirect to forwards");

        let expected = Expr::Image(
            Metadata::new(),
            Moo::new(Expr::from(Reference::new(forwards_decl))),
            Moo::new(arg),
        );
        assert_eq!(result.new_expression, expected);
    }

    #[test]
    fn image_of_perm_inverse_redirects_to_the_backwards_declaration() {
        let p = permutation_decl();
        let representation = p.get_repr::<PermutationAsFunction>().unwrap();
        let backwards_decl = representation.backwards.clone();
        drop(representation);

        let p_ref = Expr::from(Reference::new(p.clone()));
        let arg = Expr::Atomic(Metadata::new(), Atom::Literal(1.into()));
        let expr = Expr::Image(
            Metadata::new(),
            Moo::new(Expr::PermInverse(Metadata::new(), Moo::new(p_ref))),
            Moo::new(arg.clone()),
        );

        let rule = get_rule_by_name("image_perm_inverse_permutation_as_function")
            .expect("rule registered");
        let result = rule
            .apply(&expr, &SymbolTable::new())
            .expect("should redirect to backwards");

        let expected = Expr::Image(
            Metadata::new(),
            Moo::new(Expr::from(Reference::new(backwards_decl))),
            Moo::new(arg),
        );
        assert_eq!(result.new_expression, expected);
    }
}
