use conjure_cp::ast::{Expression as Expr, GroundDomain, Moo, SymbolTable};
use conjure_cp::matrix_expr;
use conjure_cp::rule_engine::{
    ApplicationError::RuleNotApplicable, ApplicationResult, RuleEffect, register_rule,
};

/// Non-scalar operands have no native backend ordering, so route `<`/`>`/`<=`/`>=` through the
/// Lex comparison machinery instead -- it already knows how to decompose every abstract type
/// (tuple, record, set, mset, ...) into a comparable form, and lex order over the whole value
/// coincides with each type's own natural ordering. Runs before `normalise_lex_gt_geq` so a
/// promoted `LexGt`/`LexGeq` gets normalised down the same way a user-written one would.
///
/// A genuine matrix operand *is* already the list `LexLt` compares element-wise, so it is passed
/// through unwrapped. Every other abstract type's own ordering rule (e.g. `lex_explicit_sets`,
/// the tuple/record comparison rules) instead expects each whole value wrapped as a singleton
/// list -- "compare these two values as if they were one-element lists" -- so it is wrapped here
/// to match.
#[register_rule("Base", 9100, [Lt, Gt, Leq, Geq])]
fn promote_abstract_cmp_to_lex(expr: &Expr, _: &SymbolTable) -> ApplicationResult {
    let (lhs, rhs) = match expr {
        Expr::Lt(_, lhs, rhs)
        | Expr::Gt(_, lhs, rhs)
        | Expr::Leq(_, lhs, rhs)
        | Expr::Geq(_, lhs, rhs) => (lhs, rhs),
        _ => return Err(RuleNotApplicable),
    };
    let Some(domain) = lhs.domain_of().or_else(|| rhs.domain_of()) else {
        return Err(RuleNotApplicable);
    };
    let Ok(ground) = domain.resolve() else {
        return Err(RuleNotApplicable);
    };
    let (is_scalar, is_matrix) = match ground.as_ref() {
        GroundDomain::Bool | GroundDomain::Int(_) => (true, false),
        GroundDomain::Matrix(_, _) => (false, true),
        _ => (false, false),
    };
    if is_scalar {
        return Err(RuleNotApplicable);
    }

    let wrap = |e: &Moo<Expr>| -> Moo<Expr> {
        if is_matrix {
            e.clone()
        } else {
            Moo::new(matrix_expr![e.as_ref().clone()])
        }
    };
    let new_expr = match expr {
        Expr::Lt(metadata, lhs, rhs) => Expr::LexLt(metadata.clone(), wrap(lhs), wrap(rhs)),
        Expr::Gt(metadata, lhs, rhs) => Expr::LexGt(metadata.clone(), wrap(lhs), wrap(rhs)),
        Expr::Leq(metadata, lhs, rhs) => Expr::LexLeq(metadata.clone(), wrap(lhs), wrap(rhs)),
        Expr::Geq(metadata, lhs, rhs) => Expr::LexGeq(metadata.clone(), wrap(lhs), wrap(rhs)),
        _ => unreachable!(),
    };
    Ok(RuleEffect::pure(new_expr))
}

#[register_rule("Base", 9000, [LexGt, LexGeq])]
fn normalise_lex_gt_geq(expr: &Expr, _: &SymbolTable) -> ApplicationResult {
    match expr {
        Expr::LexGt(metadata, lhs, rhs) => Ok(RuleEffect::pure(Expr::LexLt(
            metadata.clone(),
            rhs.clone(),
            lhs.clone(),
        ))),
        Expr::LexGeq(metadata, lhs, rhs) => Ok(RuleEffect::pure(Expr::LexLeq(
            metadata.clone(),
            rhs.clone(),
            lhs.clone(),
        ))),
        _ => Err(RuleNotApplicable),
    }
}
