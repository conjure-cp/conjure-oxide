use conjure_cp::ast::Metadata;
use conjure_cp::ast::{Expression as Expr, Moo, SymbolTable};
use conjure_cp::into_matrix_expr;
use conjure_cp::rule_engine::{
    ApplicationError::RuleNotApplicable, ApplicationResult, Reduction, register_rule,
};

/// Removes a dimension from a matrix indexing operation, if the subject of the indexing is a
/// matrix literal.
///
/// ```plain
/// [[1,2,3],[4,5,6],[7,8,9]][j,i] ~~> [[1,2,3][i],[4,5,6][i],[7,8,9][i]][j]
/// ```
#[register_rule(("Base", 2000))]
fn remove_dimension_from_matrix_indexing(expr: &Expr, _: &SymbolTable) -> ApplicationResult {
    let Expr::SafeIndex(_, subject, mut indices) = expr.clone() else {
        return Err(RuleNotApplicable);
    };

    if indices.len() < 2 {
        return Err(RuleNotApplicable);
    };

    // the indicies to use in the replacement expression.
    let outer_indices = vec![indices.pop().unwrap()];

    // the indicies to use in the inner expressions.
    let inner_indices = indices;

    let (mut es, index_domain) = Moo::unwrap_or_clone(subject)
        .unwrap_matrix_unchecked()
        .ok_or(RuleNotApplicable)?;

    for e in es.iter_mut() {
        *e = Expr::SafeIndex(Metadata::new(), Moo::new(e.clone()), inner_indices.clone());
    }

    Ok(Reduction::pure(Expr::SafeIndex(
        Metadata::new(),
        Moo::new(into_matrix_expr![es;index_domain]),
        outer_indices,
    )))
}
