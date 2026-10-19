use conjure_cp::{
    ast::{Domain, Expression as Expr, Metadata, Moo, Range, SymbolTable},
    matrix_expr,
    rule_engine::{
        ApplicationError::RuleNotApplicable, ApplicationResult, RuleEffect, register_rule,
    },
};

/// `catchUndef({v @ c}, d)` ~~> `[d, v][toInt(c)]`.
///
/// The bubble rules turn a partial expression into its total form guarded by a definedness
/// condition, which is exactly what `catchUndef` needs: pick the value where the condition holds
/// and the default where it does not.
#[register_rule("Base", 8800, [CatchUndef])]
fn catch_undef_bubble(expr: &Expr, _: &SymbolTable) -> ApplicationResult {
    let Expr::CatchUndef(_, inner, default) = expr else {
        return Err(RuleNotApplicable);
    };

    let Expr::Bubble(_, value, condition) = inner.as_ref() else {
        return Err(RuleNotApplicable);
    };

    let value = Moo::unwrap_or_clone(Moo::clone(value));
    let condition = Moo::unwrap_or_clone(Moo::clone(condition));
    let default = Moo::unwrap_or_clone(Moo::clone(default));

    Ok(RuleEffect::pure(Expr::UnsafeIndex(
        Metadata::new(),
        Moo::new(matrix_expr![default, value; Domain::int(vec![Range::Bounded(0, 1)])]),
        vec![Expr::ToInt(Metadata::new(), Moo::new(condition))],
    )))
}

/// `catchUndef(e, d)` ~~> `e` when `e` is total.
///
/// Nothing wrapped the expression in a bubble, so it is defined everywhere and the default can
/// never be reached.
#[register_rule("Base", 8700, [CatchUndef])]
fn catch_undef_total(expr: &Expr, _: &SymbolTable) -> ApplicationResult {
    let Expr::CatchUndef(_, inner, _) = expr else {
        return Err(RuleNotApplicable);
    };

    if !inner.is_safe() {
        return Err(RuleNotApplicable);
    }

    Ok(RuleEffect::pure(Moo::unwrap_or_clone(Moo::clone(inner))))
}
