use conjure_cp::ast::{
    AbstractLiteral, Atom, Expression as Expr, Literal, Metadata, Moo, PartialityAttr, SymbolTable,
};
use conjure_cp::rule_engine::{
    ApplicationError::RuleNotApplicable, ApplicationResult, RuleEffect, register_rule,
};

/// A total function defines every element of its argument domain, independently of its values
/// and of the representation chosen for it.
#[register_rule("Base", 8800, [Defined])]
fn defined_total_function(expr: &Expr, _: &SymbolTable) -> ApplicationResult {
    let Expr::Defined(_, function) = expr else {
        return Err(RuleNotApplicable);
    };
    // Preserve definedness conditions on function-valued expressions, such as an out-of-bounds
    // lookup of a total function, until their own lowering rules have run.
    if !matches!(function.as_ref(), Expr::Atomic(_, Atom::Reference(_))) {
        return Err(RuleNotApplicable);
    }
    let (attrs, domain, _) = function
        .domain_of()
        .and_then(|domain| domain.as_function())
        .ok_or(RuleNotApplicable)?;
    if attrs.partiality != PartialityAttr::Total {
        return Err(RuleNotApplicable);
    }
    let values = domain
        .resolve()
        .map_err(|_| RuleNotApplicable)?
        .values()
        .map_err(|_| RuleNotApplicable)?
        .collect();
    Ok(RuleEffect::pure(Expr::from(Literal::AbstractLiteral(
        AbstractLiteral::Set(values),
    ))))
}

/// Membership in the defined entries of a position-based function layout. Undefined slots
/// contribute nothing, including their padding values when checking the function's range.
pub(super) fn membership_from_positions(
    member: &Expr,
    count: usize,
    value: impl Fn(i32) -> Expr,
    defined: impl Fn(i32) -> Expr,
) -> Expr {
    let alternatives: Vec<Expr> = (1..=count)
        .map(|index| {
            let index = index as i32;
            Expr::And(
                Metadata::new(),
                Moo::new(conjure_cp::matrix_expr![
                    Expr::Eq(
                        Metadata::new(),
                        Moo::new(member.clone()),
                        Moo::new(value(index))
                    ),
                    defined(index),
                ]),
            )
        })
        .collect();
    Expr::Or(
        Metadata::new(),
        Moo::new(conjure_cp::into_matrix_expr![alternatives]),
    )
}
