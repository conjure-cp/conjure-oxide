use conjure_cp::ast::{Atom, Expression as Expr, GroundDomain, Metadata, Moo, Range, SymbolTable};
use conjure_cp::into_matrix_expr;
use conjure_cp::rule_engine::{ApplicationError::RuleNotApplicable, ApplicationResult, RuleEffect};
use conjure_cp::{bug, essence_expr};

/// Defer creation of a SAT integer representation until its rule is selected.
pub(super) fn defer_integer_representation(
    expr: &Expr,
    materialise: fn(&Expr, &SymbolTable) -> ApplicationResult,
) -> ApplicationResult {
    let Expr::Atomic(_, Atom::Reference(reference)) = expr else {
        return Err(RuleNotApplicable);
    };
    if reference.ptr().as_find().is_none() {
        return Err(RuleNotApplicable);
    }
    let Some(domain) = reference.resolved_domain() else {
        return Err(RuleNotApplicable);
    };
    let GroundDomain::Int(ranges) = domain.as_ref() else {
        return Err(RuleNotApplicable);
    };
    if ranges
        .iter()
        .any(|range| range.low().is_none() || range.high().is_none())
    {
        return Err(RuleNotApplicable);
    }

    let expr = expr.clone();
    Ok(RuleEffect::deferred(move |symbols| {
        materialise(&expr, symbols).expect("applicable integer representation can be materialised")
    }))
}

/// Convert an integer domain into a constraint on `subject`.
pub(super) fn int_domain_to_expr(subject: Expr, ranges: &[Range<i32>]) -> Expr {
    let subject = Moo::new(subject);
    let constraints = ranges
        .iter()
        .map(|range| match range {
            Range::Single(value) => essence_expr!(&subject = &value),
            Range::Bounded(lower, upper) => {
                essence_expr!("&subject >= &lower /\\ &subject <= &upper")
            }
            _ => bug!("Unbounded domains not supported for SAT"),
        })
        .collect();

    Expr::Or(Metadata::new(), Moo::new(into_matrix_expr!(constraints)))
}
