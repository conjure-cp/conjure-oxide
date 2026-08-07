//! `FunctionAsRelation`-specific lowering of `image(f, arg)`.
//!
//! Mirrors `FunctionExplicit`'s own `image` rule
//! (`types/function/explicit/vertical/image.rs`) almost exactly: `forward_values_matrix` is
//! indexed by plain position (`int(1..n)`), not by the function's own domain, so `arg`'s position
//! must first be found via `elementId` before indexing -- see
//! `FunctionAsRelation::State::forward_values_matrix`'s field doc for why the matrix holds plain
//! codomain values (letting this be a single-index lookup) rather than `(key, value)` tuples.
//!
//! Scoped to total functions only, matching `forward_values_matrix`'s own scope and
//! `FunctionExplicit`'s image rule for the same reason: a partial function's `image` at an
//! undefined position has no agreed meaning yet.

use super::super::FunctionAsRelation;
use conjure_cp::ast::{Atom, Expression as Expr, Metadata, Moo, Reference, SymbolTable};
use conjure_cp::rule_engine::{
    ApplicationError::RuleNotApplicable, ApplicationResult, RuleEffect, register_rule,
};
use conjure_cp::{domain_int, into_matrix_expr, range};

#[register_rule("Base", 8400, [Image])]
fn image_function_as_relation(expr: &Expr, _: &SymbolTable) -> ApplicationResult {
    let Expr::Image(_, function, arg) = expr else {
        return Err(RuleNotApplicable);
    };
    let Expr::Atomic(_, Atom::Reference(reference)) = function.as_ref() else {
        return Err(RuleNotApplicable);
    };
    let Some(representation) = reference.ptr().get_repr::<FunctionAsRelation>() else {
        return Err(RuleNotApplicable);
    };
    if representation.forward_values_matrix.is_none() {
        // Partial function: see the module doc.
        return Err(RuleNotApplicable);
    }

    let n = representation.domain_values.len() as i32;
    let domain_value_exprs: Vec<Expr> = representation
        .domain_values
        .iter()
        .cloned()
        .map(Expr::from)
        .collect();
    let domain_values_matrix = into_matrix_expr![domain_value_exprs; domain_int!(1..n)];

    let position = Expr::ElementId(Metadata::new(), Moo::new(domain_values_matrix), arg.clone());
    let values_matrix = representation
        .forward_values_matrix
        .clone()
        .unwrap_or_else(|| unreachable!("checked forward_values_matrix.is_some() above"));
    let values_ref = Expr::from(Reference::new(values_matrix));
    Ok(RuleEffect::pure(Expr::SafeIndex(
        Metadata::new(),
        Moo::new(values_ref),
        vec![position],
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
    fn image_lowers_to_an_element_id_lookup_into_the_forward_values_matrix() {
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
        <super::super::super::FunctionAsRelation as ReprRule>::init_for(&mut f).unwrap();

        let f_ref = Expr::Atomic(Metadata::new(), Atom::Reference(Reference::new(f.clone())));
        let arg = Expr::Atomic(Metadata::new(), Atom::Literal(1.into()));
        let expr = Expr::Image(Metadata::new(), Moo::new(f_ref), Moo::new(arg));

        let rule = get_rule_by_name("image_function_as_relation").expect("rule registered");
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

    #[test]
    fn image_is_not_applicable_to_a_partial_function() {
        let domain = Domain::function(
            FuncAttr::<i32> {
                size: Range::Bounded(0, 2),
                partiality: PartialityAttr::Partial,
                jectivity: JectivityAttr::None,
            },
            domain_int!(1..3),
            domain_int!(10..12),
        );
        let mut symbols = SymbolTable::new();
        let mut f = symbols.gen_find(&domain);
        <super::super::super::FunctionAsRelation as ReprRule>::init_for(&mut f).unwrap();

        let f_ref = Expr::Atomic(Metadata::new(), Atom::Reference(Reference::new(f.clone())));
        let arg = Expr::Atomic(Metadata::new(), Atom::Literal(1.into()));
        let expr = Expr::Image(Metadata::new(), Moo::new(f_ref), Moo::new(arg));

        let rule = get_rule_by_name("image_function_as_relation").expect("rule registered");
        assert!(rule.apply(&expr, &symbols).is_err());
    }
}
