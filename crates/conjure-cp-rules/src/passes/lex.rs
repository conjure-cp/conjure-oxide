use conjure_cp::ast::{Expression as Expr, SymbolTable};
use conjure_cp::rule_engine::{
    ApplicationError::RuleNotApplicable, ApplicationResult, RuleEffect, register_rule,
};

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
