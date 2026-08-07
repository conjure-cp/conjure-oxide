//! Representation-independent horizontal rules for permutations, mirroring Conjure's
//! `Rules/Horizontal/Permutation.hs`. These work purely on expression structure and don't need a
//! representation to have been chosen for their operands yet.

use conjure_cp::ast::{Expression as Expr, Metadata, Moo, SymbolTable};
use conjure_cp::rule_engine::{
    ApplicationError::RuleNotApplicable, ApplicationResult, RuleEffect, register_rule,
};

/// `image(compose(g, h), i)` = `image(g, image(h, i))`: composing then applying is the same as
/// applying the second permutation first, then the first. Mirrors Conjure's `rule_Compose_Image`.
#[register_rule("Base", 8600, [Image])]
fn image_of_compose(expr: &Expr, _: &SymbolTable) -> ApplicationResult {
    let Expr::Image(_, function, arg) = expr else {
        return Err(RuleNotApplicable);
    };
    let Expr::Compose(_, g, h) = function.as_ref() else {
        return Err(RuleNotApplicable);
    };
    let inner = Expr::Image(Metadata::new(), h.clone(), arg.clone());
    Ok(RuleEffect::pure(Expr::Image(
        Metadata::new(),
        g.clone(),
        Moo::new(inner),
    )))
}

/// `permInverse(permInverse(p))` = `p`: inverting twice cancels out.
#[register_rule("Base", 8600, [PermInverse])]
fn perm_inverse_cancellation(expr: &Expr, _: &SymbolTable) -> ApplicationResult {
    let Expr::PermInverse(_, inner) = expr else {
        return Err(RuleNotApplicable);
    };
    let Expr::PermInverse(_, innermost) = inner.as_ref() else {
        return Err(RuleNotApplicable);
    };
    Ok(RuleEffect::pure((**innermost).clone()))
}

#[cfg(test)]
mod tests {
    use conjure_cp::ast::{Atom, Literal, SymbolTable};
    use conjure_cp::rule_engine::get_rule_by_name;

    use super::*;

    fn int_lit(v: i32) -> Expr {
        Expr::Atomic(Metadata::new(), Atom::Literal(Literal::Int(v)))
    }

    #[test]
    fn image_of_compose_pushes_the_image_through_both_permutations() {
        let g = int_lit(1);
        let h = int_lit(2);
        let arg = int_lit(3);
        let expr = Expr::Image(
            Metadata::new(),
            Moo::new(Expr::Compose(
                Metadata::new(),
                Moo::new(g.clone()),
                Moo::new(h.clone()),
            )),
            Moo::new(arg.clone()),
        );

        let rule = get_rule_by_name("image_of_compose").expect("rule registered");
        let result = rule
            .apply(&expr, &SymbolTable::new())
            .expect("should push image through compose");

        let expected = Expr::Image(
            Metadata::new(),
            Moo::new(g),
            Moo::new(Expr::Image(Metadata::new(), Moo::new(h), Moo::new(arg))),
        );
        assert_eq!(result.new_expression, expected);
    }

    #[test]
    fn perm_inverse_cancellation_removes_a_double_inversion() {
        let p = int_lit(1);
        let expr = Expr::PermInverse(
            Metadata::new(),
            Moo::new(Expr::PermInverse(Metadata::new(), Moo::new(p.clone()))),
        );

        let rule = get_rule_by_name("perm_inverse_cancellation").expect("rule registered");
        let result = rule
            .apply(&expr, &SymbolTable::new())
            .expect("should cancel the double inversion");

        assert_eq!(result.new_expression, p);
    }
}
