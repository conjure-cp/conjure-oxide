use conjure_cp::ast::matrix::flatten_owned;
use conjure_cp::ast::records::RecordValue;
use conjure_cp::ast::{
    AbstractLiteral, Atom, Expression as Expr, Expression, Literal, Metadata, Moo, Name,
    eval_constant,
};
use conjure_cp::rule_engine::ApplicationError;
use conjure_cp::rule_engine::ApplicationError::RuleNotApplicable;
use conjure_cp::{bug, bug_assert_eq, essence_expr, into_matrix_expr};
use itertools::{Itertools, izip};
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

pub fn lit_to_bool(x: &Literal) -> bool {
    match x {
        Literal::Bool(b) => *b,
        Literal::Int(0) => false,
        Literal::Int(1) => true,
        _ => bug!("expected a boolean or int(0..1) literal, got {}", x),
    }
}

pub fn eval_to_usize(x: &Expr) -> usize {
    match eval_constant(x) {
        Some(Literal::Int(n)) if n >= 0 => n as usize,
        Some(lit) => bug!("Flatten expected a positive integer, got `{lit}`"),
        None => bug!("Flatten expected a constant expr, got: `{x}`"),
    }
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

pub fn collect_cmp_exprs(cmp_op: &Expr, lhs_fields: Vec<Expr>, rhs_fields: Vec<Expr>) -> Expr {
    let len = lhs_fields.len();
    bug_assert_eq!(
        len,
        rhs_fields.len(),
        "comparison of collections with different shapes!"
    );

    let mut cases = vec![Vec::<Expr>::with_capacity(len); len];
    for (i, (lhs_f, rhs_f)) in izip!(lhs_fields, rhs_fields).enumerate() {
        let eq_expr = essence_expr!(&lhs_f = &rhs_f);
        let cmp_expr = cmp_op.with_children(vec![lhs_f, rhs_f].into());

        for case in cases.iter_mut().take(i) {
            case.push(eq_expr.clone());
        }
        cases[i].push(cmp_expr);
    }

    let conjs: Vec<Expr> = cases
        .into_iter()
        .map(|c| Expr::And(Metadata::new(), Moo::new(into_matrix_expr!(c))))
        .collect();
    Expr::Or(Metadata::new(), Moo::new(into_matrix_expr!(conjs)))
}

/// If this is a matrix expression, clone its elements and get a flat iterator over them
pub fn try_flatten_matrix(expr: &Expr) -> Option<impl Iterator<Item = Expr>> {
    match expr {
        Expr::AbstractLiteral(_, m @ AbstractLiteral::Matrix(..)) => {
            Some(Box::new(flatten_owned(m.clone())) as Box<dyn Iterator<Item = Expr>>)
        }
        Expr::Atomic(
            _,
            Atom::Literal(Literal::AbstractLiteral(m @ AbstractLiteral::Matrix(..))),
        ) => {
            Some(Box::new(flatten_owned(m.clone()).map(Expr::from))
                as Box<dyn Iterator<Item = Expr>>)
        }
        _ => None,
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

pub fn as_cmp_or_lex_op(expr: &Expr) -> Option<(Moo<Expr>, Moo<Expr>)> {
    as_lex_comparison_op(expr).or_else(|| as_comparison_op(expr))
}
