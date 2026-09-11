use conjure_cp::ast::{Expression as Expr, ReturnType, SymbolTable, Typeable};
use conjure_cp::into_matrix_expr;
use conjure_cp::rule_engine::{
    ApplicationError::RuleNotApplicable, ApplicationResult, RuleEffect, register_rule,
};

/// `flatten([[a, b], [c, d]])` ~~> `[a, b, c, d]`.
///
/// Handles a flatten whose argument is written out as a matrix literal.
/// [`flatten_matrix_components`](super::components) covers a reference carrying the
/// `MatrixComponents` representation; without this rule a `sum(flatten(...))` over literal rows
/// survives to the solver adaptor, which has no atomic expression to load.
#[register_rule("Base", 8000, [Flatten])]
fn flatten_matrix_literal(expr: &Expr, _: &SymbolTable) -> ApplicationResult {
    let Expr::Flatten(_, None, matrix) = expr else {
        // TODO handle flatten with n dimension option
        return Err(RuleNotApplicable);
    };

    let elements = matrix.unwrap_list_ref().ok_or(RuleNotApplicable)?;

    // Only worth doing when there is a nested level to remove; a flat literal is already flat.
    if !elements
        .iter()
        .any(|element| element.unwrap_list_ref().is_some())
    {
        return Err(RuleNotApplicable);
    }

    let mut flat_values = Vec::new();
    for element in elements {
        collect_matrix_literal_leaves(element, &mut flat_values).ok_or(RuleNotApplicable)?;
    }

    Ok(RuleEffect::pure(into_matrix_expr![flat_values]))
}

/// Appends the leaves of nested matrix literals to `leaves`, in index order.
///
/// Returns `None` if a leaf is itself matrix-valued without being written out as a literal, such
/// as a reference to a matrix: there is nothing to flatten it into here, and treating it as a leaf
/// would leave a nested matrix behind.
fn collect_matrix_literal_leaves(expr: &Expr, leaves: &mut Vec<Expr>) -> Option<()> {
    match expr.unwrap_list_ref() {
        Some(elements) => {
            for element in elements {
                collect_matrix_literal_leaves(element, leaves)?;
            }
        }
        None => {
            if matches!(expr.return_type(), ReturnType::Matrix(_)) {
                return None;
            }
            leaves.push(expr.clone());
        }
    }
    Some(())
}
