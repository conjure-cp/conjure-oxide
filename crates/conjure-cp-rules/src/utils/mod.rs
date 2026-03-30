use conjure_cp::ast::records::RecordValue;
use conjure_cp::ast::{
    AbstractLiteral, Atom, Expression as Expr, Expression, Literal, Metadata, Moo, Name,
};
use conjure_cp::rule_engine::ApplicationError;
use conjure_cp::rule_engine::ApplicationError::RuleNotApplicable;
use conjure_cp::{essence_expr, into_matrix_expr};
use itertools::Itertools;
use uniplate::{Biplate, Uniplate};

mod to_auxvar;
#[macro_use]
mod macros;

pub use to_auxvar::*;

/// True iff `expr` is an `Atom`.
pub fn is_atom(expr: &Expr) -> bool {
    matches!(expr, Expr::Atomic(_, _))
}

/// True iff `expr` is an `Atom` or `Not(Atom)`.
pub fn is_literal(expr: &Expr) -> bool {
    match expr {
        Expr::Atomic(_, _) => true,
        Expr::Not(_, inner) => matches!(**inner, Expr::Atomic(_, _)),
        _ => false,
    }
}

/// True if `expr` is flat; i.e. it only contains atoms.
pub fn is_flat(expr: &Expr) -> bool {
    for e in expr.children() {
        if !is_atom(&e) {
            return false;
        }
    }
    true
}

/// True if the expression is a record literal
pub fn is_record_lit(expr: &Expr) -> bool {
    matches!(
        expr,
        Expr::AbstractLiteral(_, AbstractLiteral::Record(..))
            | Expr::Atomic(
                _,
                Atom::Literal(Literal::AbstractLiteral(AbstractLiteral::Record(..)))
            )
    )
}

/// True iff the expression is a tuple literal
pub fn is_tuple_lit(expr: &Expr) -> bool {
    matches!(
        expr,
        Expr::AbstractLiteral(_, AbstractLiteral::Tuple(..))
            | Expr::Atomic(
                _,
                Atom::Literal(Literal::AbstractLiteral(AbstractLiteral::Tuple(..)))
            )
    )
}

/// Returns the arity of a tuple constant expression, if this expression is one.
pub fn tuple_expr_len(expr: &Expr) -> Option<usize> {
    match expr {
        Expr::AbstractLiteral(_, AbstractLiteral::Tuple(elems)) => Some(elems.len()),
        Expr::Atomic(_, Atom::Literal(Literal::AbstractLiteral(AbstractLiteral::Tuple(elems)))) => {
            Some(elems.len())
        }
        _ => None,
    }
}

/// Get the entries of a tuple expression, if it is one
pub fn tuple_expr_entries(expr: &Expr) -> Option<Vec<Expr>> {
    match expr {
        Expr::AbstractLiteral(_, AbstractLiteral::Tuple(elems)) => Some(elems.clone()),
        Expr::Atomic(_, Atom::Literal(Literal::AbstractLiteral(AbstractLiteral::Tuple(elems)))) => {
            Some(elems.iter().cloned().map(Expr::from).collect())
        }
        _ => None,
    }
}

/// Iterate over (name, value) of a record, if the expression is one; Fields are converted to Expression
pub fn record_expr_entries<'a>(
    expr: &'a Expr,
) -> Option<Box<dyn Iterator<Item = (&'a Name, Expr)> + 'a>> {
    match expr {
        Expr::AbstractLiteral(_, AbstractLiteral::Record(entries)) => Some(Box::new(
            entries
                .iter()
                .map(|RecordValue { name, value }| (name, value.clone())),
        )),
        Expr::Atomic(
            _,
            Atom::Literal(Literal::AbstractLiteral(AbstractLiteral::Record(entries))),
        ) => {
            Some(Box::new(entries.iter().map(
                |RecordValue { name, value }| (name, value.clone().into()),
            )))
        }
        _ => None,
    }
}

/// True if the entire AST is constants.
pub fn is_all_constant(expression: &Expr) -> bool {
    for atom in expression.universe_bi() {
        match atom {
            Atom::Literal(_) => {}
            _ => {
                return false;
            }
        }
    }

    true
}

pub fn as_eq_or_neq(expr: &Expr) -> Result<(&Expr, &Expr, bool), ApplicationError> {
    match expr {
        Expression::Eq(_, left, right) => Ok((left.as_ref(), right.as_ref(), false)),
        Expression::Neq(_, left, right) => Ok((left.as_ref(), right.as_ref(), true)),
        _ => Err(RuleNotApplicable),
    }
}

pub fn collect_eq_or_neq<A, B>(neq: bool, itr: impl Iterator<Item = (A, B)>) -> Expr
where
    A: Into<Expr> + Clone,
    B: Into<Expr> + Clone,
{
    if neq {
        let neq_constraints = itr.map(|(a, b)| essence_expr!(&a != &b)).collect_vec();
        Expression::Or(
            Metadata::new(),
            Moo::new(into_matrix_expr!(neq_constraints)),
        )
    } else {
        let eq_constraints = itr.map(|(a, b)| essence_expr!(&a == &b)).collect_vec();
        Expression::And(Metadata::new(), Moo::new(into_matrix_expr!(eq_constraints)))
    }
}

pub fn as_comparison_op(expr: &Expr) -> Option<(Moo<Expr>, Moo<Expr>)> {
    match expr {
        Expr::Eq(_, lhs, rhs)
        | Expr::Neq(_, lhs, rhs)
        | Expr::Lt(_, lhs, rhs)
        | Expr::Gt(_, lhs, rhs)
        | Expr::Leq(_, lhs, rhs)
        | Expr::Geq(_, lhs, rhs) => Some((lhs.clone(), rhs.clone())),
        _ => None,
    }
}

pub fn as_lex_comparison_op(expr: &Expr) -> Option<(Moo<Expr>, Moo<Expr>)> {
    match expr {
        Expr::LexGt(_, lhs, rhs)
        | Expr::LexLt(_, lhs, rhs)
        | Expr::LexGeq(_, lhs, rhs)
        | Expr::LexLeq(_, lhs, rhs) => Some((lhs.clone(), rhs.clone())),
        _ => None,
    }
}
