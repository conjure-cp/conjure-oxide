//! `FunctionExplicit`-specific lowering of `image(f, arg)`.
//!
//! `values_matrix` is indexed by plain position (`int(1..n)`), not by the function's own
//! (possibly non-int, possibly compound) domain -- see `values_matrix`'s field doc on
//! `FunctionExplicit::State`. So `image(f, arg)` first has to find `arg`'s position among the
//! function's domain values via `elementId` (already backed by the Minion backend, the same
//! mechanism `allDifferentExcept`-style indexing uses), then index into `values_matrix` at that
//! position.
//!
//! A partial function is undefined at a position it does not define, so `image` there is wrapped
//! in a bubble on that position's definedness flag: `values_matrix` holds `padding` at undefined
//! positions, which without the bubble would quietly answer as if it were a real value.

use super::super::FunctionExplicit;
use conjure_cp::ast::{Atom, Expression as Expr, Metadata, Moo, SymbolTable};
use conjure_cp::rule_engine::{
    ApplicationError::RuleNotApplicable, ApplicationResult, RuleEffect, register_rule,
};
use conjure_cp::{domain_int, into_matrix_expr, range};

#[register_rule("Base", 8400, [Image])]
fn image_function_explicit(expr: &Expr, _: &SymbolTable) -> ApplicationResult {
    let Expr::Image(_, function, arg) = expr else {
        return Err(RuleNotApplicable);
    };
    let Expr::Atomic(_, Atom::Reference(reference)) = function.as_ref() else {
        return Err(RuleNotApplicable);
    };
    let Some(representation) = reference.ptr().get_repr::<FunctionExplicit>() else {
        return Err(RuleNotApplicable);
    };
    let n = representation.domain_values.len() as i32;
    let domain_value_exprs: Vec<Expr> = representation
        .domain_values
        .iter()
        .cloned()
        .map(Expr::from)
        .collect();
    let domain_values_matrix = into_matrix_expr![domain_value_exprs; domain_int!(1..n)];

    let position = Expr::ElementId(Metadata::new(), Moo::new(domain_values_matrix), arg.clone());
    let value = representation.value_expr_at(position.clone());

    if representation.flags_matrix.is_none() {
        return Ok(RuleEffect::pure(value));
    }

    Ok(RuleEffect::pure(Expr::Bubble(
        Metadata::new(),
        Moo::new(value),
        Moo::new(representation.defined_expr_at(position)),
    )))
}

#[cfg(test)]
mod tests {
    use conjure_cp::ast::{
        Atom, Domain, Expression as Expr, FuncAttr, JectivityAttr, Metadata, Moo, PartialityAttr,
        Range, Reference, SymbolTable,
    };
    use conjure_cp::representation::ReprRule;
    use conjure_cp::rule_engine::get_rule_by_name;
    use conjure_cp::{domain_int, range};

    #[test]
    fn image_lowers_to_an_element_id_lookup_into_the_values_matrix() {
        let domain = Domain::function(
            FuncAttr::<i32> {
                size: Range::Unbounded,
                partiality: PartialityAttr::Total,
                jectivity: JectivityAttr::None,
            },
            domain_int!(1..3),
            domain_int!(10..12),
        );
        let mut symbols = SymbolTable::new();
        let mut f = symbols.gen_find(&domain);
        <super::FunctionExplicit as ReprRule>::init_for(&mut f).unwrap();

        let f_ref = Expr::Atomic(Metadata::new(), Atom::Reference(Reference::new(f.clone())));
        let arg = Expr::Atomic(Metadata::new(), Atom::Literal(1.into()));
        let expr = Expr::Image(Metadata::new(), Moo::new(f_ref), Moo::new(arg));

        let rule = get_rule_by_name("image_function_explicit").expect("rule registered");
        let result = rule.apply(&expr, &symbols).expect("should lower image");

        let Expr::SafeIndex(_, matrix, indices) = &result.new_expression else {
            panic!("expected a SafeIndex, got {}", result.new_expression);
        };
        assert!(matches!(
            matrix.as_ref(),
            Expr::Atomic(_, Atom::Reference(_))
        ));
        assert_eq!(indices.len(), 1);
        assert!(matches!(indices[0], Expr::ElementId(_, _, _)));
    }
}
