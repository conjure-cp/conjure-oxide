use conjure_cp::ast::records::RecordValue;
use conjure_cp::ast::{AbstractLiteral, Atom, Expression as Expr, Literal, Name};
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

/// Returns the arity of a tuple constant expression, if this expression is one.
pub fn constant_tuple_len(expr: &Expr) -> Option<usize> {
    match expr {
        Expr::AbstractLiteral(_, AbstractLiteral::Tuple(elems)) => Some(elems.len()),
        Expr::Atomic(_, Atom::Literal(Literal::AbstractLiteral(AbstractLiteral::Tuple(elems)))) => {
            Some(elems.len())
        }
        _ => None,
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

/// Converts a vector of expressions to a vector of atoms.
///
/// # Returns
///
/// `Some(Vec<Atom>)` if the vectors direct children expressions are all atomic, otherwise `None`.
#[allow(dead_code)]
pub fn expressions_to_atoms(exprs: &Vec<Expr>) -> Option<Vec<Atom>> {
    let mut atoms: Vec<Atom> = vec![];
    for expr in exprs {
        let Expr::Atomic(_, atom) = expr else {
            return None;
        };
        atoms.push(atom.clone());
    }

    Some(atoms)
}
