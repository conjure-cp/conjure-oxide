use conjure_core::{
    ast::{Expression as Expr, SymbolTable},
    into_matrix_expr,
    metadata::Metadata,
    rule_engine::{
        ApplicationError::RuleNotApplicable, ApplicationResult, Reduction, register_rule,
    },
};

#[register_rule(("Base", 2000))]
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

#[register_rule(("Base", 8000))]
fn comprehension_move_static_guard_into_generators(
    expr: &Expr,
    _: &SymbolTable,
) -> ApplicationResult {
    let Expr::Comprehension(_, comprehension) = expr else {
        return Err(RuleNotApplicable);
    };

    let mut comprehension = comprehension.clone();
    let Expr::Imply(_, guard, expr) = comprehension.clone().return_expression() else {
        return Err(RuleNotApplicable);
    };

    if comprehension.add_induction_guard(*guard) {
        comprehension.replace_return_expression(*expr);
        Ok(Reduction::pure(Expr::Comprehension(
            Metadata::new(),
            comprehension,
        )))
    } else {
        Err(RuleNotApplicable)
    }
}
