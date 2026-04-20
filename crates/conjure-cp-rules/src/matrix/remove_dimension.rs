use crate::guard;
use crate::utils::{as_list_combining_op, is_matrix_lit};
use conjure_cp::ast::Metadata;
use conjure_cp::ast::{Expression as Expr, Moo, SymbolTable};
use conjure_cp::into_matrix_expr;
use conjure_cp::rule_engine::{
    ApplicationError::RuleNotApplicable, ApplicationResult, Reduction, register_rule,
};
use uniplate::Biplate;

/// Removes a dimension from a matrix indexing operation, if the subject of the indexing is a
/// matrix literal.
///
/// ```plain
/// [[1,2,3],[4,5,6],[7,8,9]][j,i] ~~> [[1,2,3][i],[4,5,6][i],[7,8,9][i]][j]
/// ```
#[register_rule(("Base", 2000))]
fn remove_dimension_from_matrix_indexing(expr: &Expr, _: &SymbolTable) -> ApplicationResult {
    let (subject, mut indices, safe) = match expr {
        Expr::SafeIndex(_, subject, indices) => (subject.clone(), indices.clone(), true),
        Expr::UnsafeIndex(_, subject, indices) => (subject.clone(), indices.clone(), false),
        _ => return Err(RuleNotApplicable),
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
        if safe {
            *e = Expr::SafeIndex(Metadata::new(), Moo::new(e.clone()), inner_indices.clone());
        } else {
            *e = Expr::UnsafeIndex(Metadata::new(), Moo::new(e.clone()), inner_indices.clone());
        }
    }

    let new_expr = if safe {
        Expr::SafeIndex(
            Metadata::new(),
            Moo::new(into_matrix_expr![es;index_domain]),
            outer_indices,
        )
    } else {
        Expr::UnsafeIndex(
            Metadata::new(),
            Moo::new(into_matrix_expr![es;index_domain]),
            outer_indices,
        )
    };
    Ok(Reduction::pure(new_expr))
}

/// Removes a dimension from a nested matrix indexing operation, if the innermost subject is a
/// matrix literal.
///
/// The outer index is brought inside the literal, applied to each element, and the inner index
/// becomes the new outer index.
///
/// ```plain
/// [[1,2,3],[4,5,6],[7,8,9]][j][i] ~~> [[1,2,3][i],[4,5,6][i],[7,8,9][i]][j]
/// ```
#[register_rule(("Base", 2000))]
fn remove_dimension_from_multiple_matrix_indexing(
    expr: &Expr,
    _: &SymbolTable,
) -> ApplicationResult {
    // Match the outer indexing operation.
    let (outer_subject, outer_indices, outer_safe) = match expr {
        Expr::SafeIndex(_, subject, indices) => (subject, indices, true),
        Expr::UnsafeIndex(_, subject, indices) => (subject, indices, false),
        _ => return Err(RuleNotApplicable),
    };

    // The subject of the outer index must itself be an indexing operation.
    let (inner_subject, inner_indices, inner_safe) = match outer_subject.as_ref() {
        Expr::SafeIndex(_, subject, indices) => (subject.clone(), indices.clone(), true),
        Expr::UnsafeIndex(_, subject, indices) => (subject.clone(), indices.clone(), false),
        _ => return Err(RuleNotApplicable),
    };

    // The innermost subject must be a matrix literal.
    let (mut es, index_domain) = Moo::unwrap_or_clone(inner_subject)
        .unwrap_matrix_unchecked()
        .ok_or(RuleNotApplicable)?;

    // Apply the outer indices to each element of the matrix literal.
    for e in es.iter_mut() {
        if outer_safe {
            *e = Expr::SafeIndex(Metadata::new(), Moo::new(e.clone()), outer_indices.clone());
        } else {
            *e = Expr::UnsafeIndex(Metadata::new(), Moo::new(e.clone()), outer_indices.clone());
        }
    }

    // Reconstruct with the inner indices as the new outer index.
    let new_expr = if inner_safe {
        Expr::SafeIndex(
            Metadata::new(),
            Moo::new(into_matrix_expr![es;index_domain]),
            inner_indices,
        )
    } else {
        Expr::UnsafeIndex(
            Metadata::new(),
            Moo::new(into_matrix_expr![es;index_domain]),
            inner_indices,
        )
    };

    Ok(Reduction::pure(new_expr))
}

/// Distributes a `RecordField` operation over a matrix indexing expression whose subject is a
/// matrix literal.
///
/// ```plain
/// RecordField([a, b, c][i], "f") ~~> [RecordField(a, "f"), RecordField(b, "f"), RecordField(c, "f")][i]
/// ```
#[register_rule(("Base", 2000))]
fn record_field_distribute_over_matrix_index(expr: &Expr, _: &SymbolTable) -> ApplicationResult {
    let Expr::RecordField(_, subject, field_name) = expr else {
        return Err(RuleNotApplicable);
    };

    // The subject of the RecordField must be an indexing operation.
    let (index_subject, indices, safe) = match subject.as_ref() {
        Expr::SafeIndex(_, subject, indices) => (subject.clone(), indices.clone(), true),
        Expr::UnsafeIndex(_, subject, indices) => (subject.clone(), indices.clone(), false),
        _ => return Err(RuleNotApplicable),
    };

    // The subject of the indexing must be a matrix literal.
    let (mut es, index_domain) = Moo::unwrap_or_clone(index_subject)
        .unwrap_matrix_unchecked()
        .ok_or(RuleNotApplicable)?;

    // Apply RecordField to each element of the matrix literal.
    for e in es.iter_mut() {
        *e = Expr::RecordField(Metadata::new(), Moo::new(e.clone()), field_name.clone());
    }

    // Reconstruct the indexing expression with the modified matrix.
    let new_expr = if safe {
        Expr::SafeIndex(
            Metadata::new(),
            Moo::new(into_matrix_expr![es;index_domain]),
            indices,
        )
    } else {
        Expr::UnsafeIndex(
            Metadata::new(),
            Moo::new(into_matrix_expr![es;index_domain]),
            indices,
        )
    };

    Ok(Reduction::pure(new_expr))
}

/// Distribute a 1D list-combining operation over a 2D matrix literal indexing
///
/// ```plain
/// sum([[1, 2], [3, 4]][x])
/// ~>
/// [sum([1, 2]), sum([3, 4])][x]
/// ```
#[register_rule(("Base", 1000))]
fn distribute_list_combining_op(expr: &Expr, _: &SymbolTable) -> ApplicationResult {
    guard!(
        let Some(inner) = as_list_combining_op(expr)              &&
        let Expr::SafeIndex(_, subject, indices) = inner.as_ref() &&
        indices.len() == 1                                        &&
        let Some(idx) = indices.first()                           &&
        let Some(elems) = subject.unwrap_list()
        else {
            return Err(RuleNotApplicable);
        }
    );

    let mut new_elems = Vec::with_capacity(elems.len());
    for elem in elems {
        if !is_matrix_lit(&elem) {
            return Err(RuleNotApplicable);
        }
        let new_elem = expr.with_children_bi([Moo::new(elem)].into());
        new_elems.push(new_elem);
    }

    let new_expr = Expr::SafeIndex(
        Metadata::new(),
        Moo::new(into_matrix_expr!(new_elems)),
        vec![idx.clone()],
    );
    Ok(Reduction::pure(new_expr))
}
