use crate::guard;
use crate::utils::eval_to_usize;
use conjure_cp::ast::matrix::partial_flatten;
use conjure_cp::ast::{
    AbstractLiteral, Atom, Expression as Expr, GroundDomain, Literal, Metadata, SymbolTable, matrix,
};
use conjure_cp::rule_engine::{
    ApplicationError::RuleNotApplicable, ApplicationResult, Reduction, register_rule,
};

/// Turn an index into a flattened matrix expression directly into the fully qualified index.
///
/// E.g. instead of transforming flatten(m)[1] ~> [m[1,1],m[1,2],..][1],
///                          do: flatten(m)[1] ~> m[1,1]
#[register_rule(("Base", 8001))]
fn indexed_flatten_matrix(expr: &Expr, _: &SymbolTable) -> ApplicationResult {
    let (subject, index) = match expr {
        Expr::SafeIndex(_, subj, idx) | Expr::UnsafeIndex(_, subj, idx) => (subj, idx),
        _ => return Err(RuleNotApplicable),
    };
    let Expr::Flatten(_, n, matrix) = subject.as_ref() else {
        return Err(RuleNotApplicable);
    };

    if n.is_some() || index.len() != 1 {
        // TODO handle flatten with n dimension option
        return Err(RuleNotApplicable);
    }

    // get the actual number of the index
    let Expr::Atomic(_, Atom::Literal(Literal::Int(index))) = index[0] else {
        return Err(RuleNotApplicable);
    };

    // resolve index domains so that we can enumerate them later
    let dom = matrix
        .domain_of()
        .and_then(|dom| dom.resolve().ok())
        .ok_or(RuleNotApplicable)?;

    let GroundDomain::Matrix(_, index_domains) = dom.as_ref() else {
        return Err(RuleNotApplicable);
    };

    let flat_index = matrix::flat_index_to_full_index(index_domains, (index - 1) as u64);
    let flat_index: Vec<Expr> = flat_index.into_iter().map(Into::into).collect();

    // This must be unsafe since we are using a possibly unsafe flat index.
    // TODO: this can be made safe if matrix::flat_index_to_full_index fails out of bounds
    let new_expr = Expr::UnsafeIndex(Metadata::new(), matrix.clone(), flat_index);
    Ok(Reduction::pure(new_expr))
}

/// Flatten a matrix expression
/// ```plain
/// flatten(1, [[a, b], [c, d]])
/// ~>
/// [a, b, c, d]
/// ```
#[register_rule(("Base", 2001))]
fn flatten_matrix_expr(expr: &Expr, _: &SymbolTable) -> ApplicationResult {
    guard!(
        let Expr::Flatten(_, dims, subj) = expr                                       &&
        let Expr::AbstractLiteral(_, m @ AbstractLiteral::Matrix(..)) = subj.as_ref()
        else {
            return Err(RuleNotApplicable);
        }
    );

    let n = dims.as_ref().map(|x| eval_to_usize(x)).unwrap_or(0);
    let new_expr = partial_flatten(n, m.clone());

    if new_expr.eq(m) {
        return Err(RuleNotApplicable);
    }
    Ok(Reduction::pure(new_expr.into()))
}

/// Flatten a matrix literal
/// ```plain
/// flatten(1, [[1, 2], [3, 4]])
/// ~>
/// [1, 2, 3, 4]
/// ```
#[register_rule(("Base", 2001))]
fn flatten_matrix_lit(expr: &Expr, _: &SymbolTable) -> ApplicationResult {
    guard!(
        let Expr::Flatten(_, dims, subj) = expr                             &&
        let Expr::Atomic(_, Atom::Literal(lit)) = subj.as_ref()             &&
        let Literal::AbstractLiteral(m @ AbstractLiteral::Matrix(..)) = lit
        else {
            return Err(RuleNotApplicable);
        }
    );

    let n = dims.as_ref().map(|x| eval_to_usize(x)).unwrap_or(0);
    let new_lit = partial_flatten(n, m.clone());
    if new_lit.eq(m) {
        return Err(RuleNotApplicable);
    }

    let new_expr = Expr::from(Atom::from(Literal::from(new_lit)));
    Ok(Reduction::pure(new_expr))
}
