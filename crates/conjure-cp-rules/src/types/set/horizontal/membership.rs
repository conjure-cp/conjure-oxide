use conjure_cp::{
    ast::{AbstractLiteral, Atom, Expression as Expr, Literal, Metadata, Moo, SymbolTable},
    into_matrix_expr,
    rule_engine::{
        ApplicationError::RuleNotApplicable, ApplicationResult, RuleEffect, register_rule,
    },
};

/// `x in {a, b, c}` ~~> `or([x = a, x = b, x = c])`.
///
/// Membership in a set written out in full does not depend on a representation, so this holds for
/// any element type. Minion's `w-inset` covers the common case of an atomic member and integer
/// elements, so that case is left to `in_set`; this rule picks up everything else, such as a tuple
/// in a set of tuples.
#[register_rule("Base", 8700, [In])]
fn membership_in_set_literal(expr: &Expr, _: &SymbolTable) -> ApplicationResult {
    let Expr::In(_, member, collection) = expr else {
        return Err(RuleNotApplicable);
    };

    let elements = set_literal_elements(collection).ok_or(RuleNotApplicable)?;

    if minion_w_inset_applies(&elements) {
        return Err(RuleNotApplicable);
    }

    if elements.is_empty() {
        return Ok(RuleEffect::pure(false.into()));
    }

    let disjuncts = elements
        .into_iter()
        .map(|element| {
            Expr::Eq(
                Metadata::new(),
                Moo::new(Moo::unwrap_or_clone(Moo::clone(member))),
                Moo::new(element),
            )
        })
        .collect();

    Ok(RuleEffect::pure(Expr::Or(
        Metadata::new(),
        Moo::new(into_matrix_expr![disjuncts]),
    )))
}

/// The elements of a set written out in full, in either the literal or the expression form.
fn set_literal_elements(collection: &Expr) -> Option<Vec<Expr>> {
    match collection {
        Expr::Atomic(
            _,
            Atom::Literal(Literal::AbstractLiteral(AbstractLiteral::Set(elements))),
        ) => Some(
            elements
                .iter()
                .map(|literal| Expr::Atomic(Metadata::new(), Atom::Literal(literal.clone())))
                .collect(),
        ),
        Expr::AbstractLiteral(_, AbstractLiteral::Set(elements)) => Some(elements.clone()),
        _ => None,
    }
}

/// Whether `in_set` can lower this membership to Minion's `w-inset` instead.
///
/// Only the elements are checked, not the member: the member may still be an index or another
/// expression that later rules reduce to an atom, and pre-empting that here would replace a native
/// `w-inset` with a disjunction.
fn minion_w_inset_applies(elements: &[Expr]) -> bool {
    elements.iter().all(|element| {
        matches!(element, Expr::Atomic(_, Atom::Literal(literal)) if i32::try_from(literal).is_ok())
    })
}
