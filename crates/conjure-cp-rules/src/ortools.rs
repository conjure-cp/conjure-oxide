use crate::utils::to_aux_var;
use conjure_cp::ast::Moo;
use conjure_cp::{
    ast::AbstractLiteral,
    ast::Metadata,
    ast::{Expression as Expr, SymbolTable},
    rule_engine::{ApplicationResult, Reduction, register_rule, register_rule_set},
    settings::SolverFamily,
};

register_rule_set!("OrToolsCpSat", ("Base"), |f: &SolverFamily| {
    matches!(f, SolverFamily::OrToolsCpSat)
});

#[register_rule("OrToolsCpSat", 4200, [Or, And])]
fn flatten_logical(expr: &Expr, symbols: &SymbolTable) -> ApplicationResult {
    use conjure_cp::rule_engine::ApplicationError::RuleNotApplicable;

    if !matches!(expr, Expr::Or(_, _) | Expr::And(_, _)) {
        return Err(RuleNotApplicable);
    }

    let mut symbols = symbols.clone();
    let mut new_tops: Vec<Expr> = vec![];

    // Get the inner expression of Or/And (which is the matrix/list)
    let inner_expr = match expr {
        Expr::Or(_, inner) | Expr::And(_, inner) => inner.as_ref(),
        _ => unreachable!(),
    };

    // If it's a matrix literal, we want to flatten its elements
    let Some((es, index_domain)) = inner_expr.clone().unwrap_matrix_unchecked() else {
        return Err(RuleNotApplicable);
    };

    let mut new_es = es;
    let mut num_changed = 0;

    for e in new_es.iter_mut() {
        if let Some(aux_info) = to_aux_var(e, &symbols) {
            symbols = aux_info.symbols();
            new_tops.push(aux_info.top_level_expr());
            *e = aux_info.as_expr();
            num_changed += 1;
        }
    }

    if num_changed == 0 {
        return Err(RuleNotApplicable);
    }

    let new_matrix = Expr::AbstractLiteral(
        Metadata::new(),
        AbstractLiteral::Matrix(new_es, index_domain),
    );

    let new_expr = match expr {
        Expr::Or(meta, _) => Expr::Or(meta.clone(), Moo::new(new_matrix)),
        Expr::And(meta, _) => Expr::And(meta.clone(), Moo::new(new_matrix)),
        _ => unreachable!(),
    };

    Ok(Reduction::new(new_expr, new_tops, symbols))
}
