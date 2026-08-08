//! `PermutationAsFunction`-specific lowering of `|p|` (cardinality, i.e. `numMoved`).

use super::super::PermutationAsFunction;
use conjure_cp::ast::{Atom, Expression as Expr, SymbolTable};
use conjure_cp::rule_engine::{
    ApplicationError::RuleNotApplicable, ApplicationResult, RuleEffect, register_rule,
};

/// `|p|` for a directly-referenced `PermutationAsFunction`-represented permutation reuses the
/// same `sum([ toInt(i != image(forwards, i)) | i : innerDomain ])` expression the `numMoved`
/// structural constraint is already built from (`State::cardinality_expr`).
#[register_rule("Base", 8500, [Card])]
fn card_permutation_as_function(expr: &Expr, _: &SymbolTable) -> ApplicationResult {
    let Expr::Card(_, inner) = expr else {
        return Err(RuleNotApplicable);
    };
    let Expr::Atomic(_, Atom::Reference(reference)) = inner.as_ref() else {
        return Err(RuleNotApplicable);
    };
    let Some(representation) = reference.ptr().get_repr::<PermutationAsFunction>() else {
        return Err(RuleNotApplicable);
    };

    Ok(RuleEffect::pure(representation.cardinality_expr()))
}

#[cfg(test)]
mod tests {
    use super::*;
    use conjure_cp::ast::{Domain, Metadata, Moo, PermutationAttr, Range, Reference};
    use conjure_cp::representation::ReprRule;
    use conjure_cp::rule_engine::get_rule_by_name;
    use conjure_cp::{domain_int, range};

    #[test]
    fn card_lowers_to_a_sum_over_the_inner_domain() {
        let domain = Domain::permutation(
            PermutationAttr::<i32> {
                num_moved: Range::Unbounded,
            },
            domain_int!(1..3),
        );
        let mut symbols = SymbolTable::new();
        let mut p = symbols.gen_find(&domain);
        <PermutationAsFunction as ReprRule>::init_for(&mut p).unwrap();

        let expr = Expr::Card(Metadata::new(), Moo::new(Expr::from(Reference::new(p))));

        let rule = get_rule_by_name("card_permutation_as_function").expect("registered");
        let result = rule
            .apply(&expr, &SymbolTable::new())
            .expect("should lower");
        // cardinality_expr() builds a fresh quantified variable each call, so comparing against a
        // second call's output directly would spuriously fail on the generated declaration's own
        // identity -- check the shape instead: a Sum wrapping a comprehension.
        assert!(matches!(result.new_expression, Expr::Sum(..)));
        let Expr::Sum(_, inner) = &result.new_expression else {
            unreachable!()
        };
        assert!(matches!(**inner, Expr::Comprehension(..)));
    }
}
