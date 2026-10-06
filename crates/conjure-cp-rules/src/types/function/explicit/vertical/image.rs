//! `FunctionExplicit`-specific lowering of `image(f, arg)`.
//!
//! `values_matrix` is indexed by plain position (`int(1..n)`), not by the function's own
//! (possibly non-int, possibly compound) domain -- see `values_matrix`'s field doc on
//! `FunctionExplicit::State`. So `image(f, arg)` first has to find `arg`'s position among the
//! function's domain values via `indexOf`, then index into `values_matrix` at that position.
//! Minion lowers the inverse lookup natively; SAT scalar domains reuse element selection.
//!
//! Out-of-domain arguments retain a membership guard. A partial function is also undefined at a
//! position it does not define, so `image` there is wrapped
//! in a bubble on that position's definedness flag: `values_matrix` holds `padding` at undefined
//! positions, which without the bubble would quietly answer as if it were a real value.

use super::super::FunctionExplicit;
use conjure_cp::ast::{Atom, Expression as Expr, GroundDomain, Metadata, Moo, SymbolTable};
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

    let position = Expr::IndexOf(Metadata::new(), Moo::new(domain_values_matrix), arg.clone());
    let value = representation.value_expr_at(position.clone());

    let domain = function
        .domain_of()
        .and_then(|domain| domain.resolve().ok())
        .ok_or(RuleNotApplicable)?;
    let GroundDomain::Function(_, argument_domain, _) = domain.as_ref() else {
        return Err(RuleNotApplicable);
    };
    let argument_inside = arg
        .domain_of()
        .and_then(|domain| domain.resolve().ok())
        .is_some_and(|domain| {
            domain
                .intersect(argument_domain)
                .is_ok_and(|intersection| intersection == *domain.as_ref())
        });
    if representation.flags_matrix.is_none() && argument_inside {
        return Ok(RuleEffect::pure(value));
    }

    // An absent scalar argument may map to a valid internal position through identity padding.
    // Keep membership in the original function domain separate from that position lookup.
    let mut conditions = Vec::new();
    if !argument_inside {
        let candidates = representation
            .domain_values
            .iter()
            .map(|candidate| {
                Expr::Eq(
                    Metadata::new(),
                    arg.clone(),
                    Moo::new(Expr::from(candidate.clone())),
                )
            })
            .collect();
        conditions.push(Expr::Or(
            Metadata::new(),
            Moo::new(into_matrix_expr![candidates]),
        ));
    }
    if representation.flags_matrix.is_some() {
        conditions.push(representation.defined_expr_at(position));
    }
    let condition = if conditions.len() == 1 {
        conditions.pop().unwrap()
    } else {
        Expr::And(Metadata::new(), Moo::new(into_matrix_expr![conditions]))
    };

    Ok(RuleEffect::pure(Expr::Bubble(
        Metadata::new(),
        Moo::new(value),
        Moo::new(condition),
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
    fn image_lowers_to_an_index_of_lookup_into_the_values_matrix() {
        let domain = Domain::function(
            FuncAttr::<i32> {
                representation: None,
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
        assert!(matches!(indices[0], Expr::IndexOf(_, _, _)));
    }

    #[test]
    fn image_outside_original_domain_is_undefined_even_at_valid_internal_positions() {
        let domain = Domain::function(
            FuncAttr::<i32> {
                representation: None,
                size: Range::Unbounded,
                partiality: PartialityAttr::Total,
                jectivity: JectivityAttr::None,
            },
            Domain::int(vec![Range::Single(-2), Range::Single(3)]),
            domain_int!(10..20),
        );
        let mut symbols = SymbolTable::new();
        let mut f = symbols.gen_find(&domain);
        <super::FunctionExplicit as ReprRule>::init_for(&mut f).unwrap();
        let function = Expr::from(Reference::new(f));
        for argument in [-3, 0, 1, 2, 4] {
            let expr = Expr::Image(
                Metadata::new(),
                Moo::new(function.clone()),
                Moo::new(argument.into()),
            );
            let lowered = super::image_function_explicit(&expr, &symbols)
                .unwrap()
                .new_expression;
            let Expr::Bubble(_, _, condition) = lowered else {
                panic!("out-of-domain image must retain its definedness guard");
            };
            assert_eq!(
                conjure_cp::ast::eval_constant(&condition),
                Some(false.into())
            );
        }
    }
}
