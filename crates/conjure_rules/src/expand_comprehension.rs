use conjure_core::{
    ast::{Expression as Expr, SymbolTable},
    into_matrix_expr,
    rule_engine::{
        register_rule, ApplicationError::RuleNotApplicable, ApplicationResult, Reduction,
    },
};

#[register_rule(("Base", 6000))]
fn expand_comprehension(expr: &Expr, symbols: &SymbolTable) -> ApplicationResult {
    let Expr::Comprehension(_, comprehension) = expr else {
        return Err(RuleNotApplicable);
    };

    // TODO: check what kind of error this throws and maybe panic

    let mut symbols = symbols.clone();
    let results = comprehension
        .as_ref()
        .clone()
        .solve_with_minion(&mut symbols)
        .or(Err(RuleNotApplicable))?;

    Ok(Reduction::with_symbols(into_matrix_expr!(results), symbols))
}
