//! Identity-default selection shares ordinary indexing and definedness handling.
use conjure_cp::ast::{
    AbstractLiteral, Atom, Expression as Expr, Literal, Metadata, Moo, SymbolTable,
};
use conjure_cp::rule_engine::{
    ApplicationError::RuleNotApplicable, ApplicationResult, RuleEffect, register_rule,
};

#[register_rule("Base", 19000, [ElementId])]
fn element_id_to_identity_lookup(expr: &Expr, _: &SymbolTable) -> ApplicationResult {
    let Expr::ElementId(_, matrix, index) = expr else {
        return Err(RuleNotApplicable);
    };
    // Minion literal lookups must wait for resolved labels and its total element lowering.
    if conjure_cp::settings::try_current_solver_family()
        == Some(conjure_cp::settings::SolverFamily::Minion)
        && matches!(
            matrix.as_ref(),
            Expr::AbstractLiteral(_, AbstractLiteral::Matrix(..))
                | Expr::Atomic(
                    _,
                    Atom::Literal(Literal::AbstractLiteral(AbstractLiteral::Matrix(..)))
                )
        )
    {
        return Err(RuleNotApplicable);
    }
    if matches!(matrix.as_ref(), Expr::Atomic(_, Atom::Literal(Literal::AbstractLiteral(AbstractLiteral::Matrix(entries, _)))) if entries.is_empty())
        || matrix
            .unwrap_matrix_unchecked_ref()
            .is_some_and(|(entries, _)| entries.is_empty())
    {
        let default = if index.domain_of().is_some_and(|domain| domain.is_bool()) {
            Expr::ToInt(Metadata::new(), index.clone())
        } else {
            (**index).clone()
        };
        return Ok(RuleEffect::pure(default));
    }
    // Convert Boolean entries before indexing so Boolean bubbling cannot turn an undefined
    // lookup into false before catchUndef sees its numeric definedness condition.
    let domain = matrix.domain_of().ok_or(RuleNotApplicable)?;
    let (entries_domain, indices) = domain.as_matrix().ok_or(RuleNotApplicable)?;
    let subject = if entries_domain.is_bool() {
        let (entries, labels) =
            if let Some((entries, labels)) = matrix.unwrap_matrix_unchecked_ref() {
                (entries.to_vec(), labels.clone())
            } else {
                let [labels] = indices.as_slice() else {
                    return Err(RuleNotApplicable);
                };
                let entries = labels
                    .resolve()
                    .map_err(|_| RuleNotApplicable)?
                    .values()
                    .map_err(|_| RuleNotApplicable)?
                    .map(|label| {
                        conjure_cp::ast::matrix::safe_index_optimised((**matrix).clone(), label)
                    })
                    .collect::<Option<Vec<_>>>()
                    .ok_or(RuleNotApplicable)?;
                (entries, labels.clone())
            };
        let entries = entries
            .into_iter()
            .map(|entry| Expr::ToInt(Metadata::new(), Moo::new(entry)))
            .collect();
        Moo::new(conjure_cp::into_matrix_expr![entries; labels])
    } else {
        matrix.clone()
    };
    let lookup = Expr::UnsafeIndex(Metadata::new(), subject, vec![(**index).clone()]);
    let numeric = |expression: Expr| {
        let domain = expression.domain_of()?;
        if domain.is_bool() {
            Some(Expr::ToInt(Metadata::new(), Moo::new(expression)))
        } else if domain.is_int() {
            Some(expression)
        } else {
            None
        }
    };
    let lookup = numeric(lookup).ok_or(RuleNotApplicable)?;
    let default = numeric((**index).clone()).ok_or(RuleNotApplicable)?;
    Ok(RuleEffect::pure(Expr::CatchUndef(
        Metadata::new(),
        Moo::new(lookup),
        Moo::new(default),
    )))
}
