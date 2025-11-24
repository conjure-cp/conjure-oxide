use conjure_cp::ast::{Expression as Expr, SymbolTable};
use conjure_cp::rule_engine::{ApplicationError, ApplicationResult, Reduction, register_rule};

use ApplicationError::RuleNotApplicable;

#[register_rule(("Base", 9000))]
fn normalise_lex_gt_geq(expr: &Expr, _: &SymbolTable) -> ApplicationResult {
    match expr {
        Expr::LexGt(metadata, a, b) => Ok(Reduction::pure(Expr::LexLt(
            metadata.clone_dirty(),
            b.clone(),
            a.clone(),
        ))),
        Expr::LexGeq(metadata, a, b) => Ok(Reduction::pure(Expr::LexLeq(
            metadata.clone_dirty(),
            b.clone(),
            a.clone(),
        ))),
        _ => Err(RuleNotApplicable),
    }
}

#[register_rule(("Minion", 2000))]
fn flatten_lex_lt_leq(expr: &Expr, _: &SymbolTable) -> ApplicationResult {
    let (a, b) = match expr {
        Expr::LexLt(_, a, b) | Expr::LexGt(_, a, b) => Some((a.unwrap_list(), b.unwrap_list())),
        _ => None,
    }
    .ok_or(RuleNotApplicable)?;
}
