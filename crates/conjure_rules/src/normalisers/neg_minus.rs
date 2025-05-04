//! Normalising rules for negations and minus operations.
//!
//!
//! ```text
//! 1. --x ~> x  (eliminate_double_negation)
//! 2. -( x + y ) ~> -x + -y (distribute_negation_over_addition)
//! 3. x - b ~>  x + -b (minus_to_sum)
//! 4. -(x*y) ~> -1 * x * y (simplify_negation_of_product
//! ```
//!
//! ## Rationale for `x - y ~> x + -y`
//!
//! I normalise `Minus` expressions into sums of negations.
//!
//! Once all negations are in one sum expression, partial evaluation becomes easier, and we can do
//! further normalisations like collecting like terms, removing nesting, and giving things an
//! ordering.
//!
//! Converting to a sum is especially helpful for converting the model to Minion as:
//!
//! 1. normalise_associative_commutative concatenates nested sums, reducing the
//!    amount of flattening we need to do to convert this to Minion (reducing the number of
//!    auxiliary variables needed).
//!
//! 2. A sum of variables with constant coefficients can be trivially converted into the
//!    weightedsumgeq and weightedsumleq constraints. A negated number is just a number
//!    with a coefficient of -1.

use conjure_core::{
    ast::{Expression as Expr, ReturnType::Set, SymbolTable, Typeable},
    metadata::Metadata,
    rule_engine::{
        register_rule, ApplicationError::RuleNotApplicable, ApplicationResult, Reduction,
    },
};
use std::collections::VecDeque;
use uniplate::{Biplate, Uniplate as _};
use Expr::*;

/// Eliminates double negation
///
/// ```text
/// --x ~> x
/// ```
#[register_rule(("Base", 8400))]
fn elmininate_double_negation(expr: &Expr, _: &SymbolTable) -> ApplicationResult {
    match expr {
        Neg(_, a) if matches!(**a, Neg(_, _)) => Ok(Reduction::pure(a.children()[0].clone())),
        _ => Err(RuleNotApplicable),
    }
}

/// Distributes negation over sums
///
/// ```text
/// -(x + y) ~> -x + -y
/// ```
#[register_rule(("Base", 8400))]
fn distribute_negation_over_sum(expr: &Expr, _: &SymbolTable) -> ApplicationResult {
    let inner_expr = match expr {
        Neg(_, e) if matches!(**e, Sum(_, _)) => Ok(*e.clone()),
        _ => Err(RuleNotApplicable),
    }?;

    let mut child_vecs: VecDeque<Vec<Expr>> = inner_expr.children_bi();

    if child_vecs.is_empty() {
        return Err(RuleNotApplicable);
    }

    for child in child_vecs[0].iter_mut() {
        *child = Neg(Metadata::new(), Box::new(child.clone()))
    }

    Ok(Reduction::pure(inner_expr.with_children_bi(child_vecs)))
}

/// Simplifies the negation of a product
///
/// ```text
/// -(x * y) ~> -1 * x * y
/// ```
#[register_rule(("Base", 8400))]
fn simplify_negation_of_product(expr: &Expr, _: &SymbolTable) -> ApplicationResult {
    let Neg(_, expr1) = expr.clone() else {
        return Err(RuleNotApplicable);
    };

    let Product(_, mut factors) = *expr1 else {
        return Err(RuleNotApplicable);
    };

    factors.push(Expr::Atomic(Metadata::new(), Atom::Literal(Lit::Int(-1))));

    Ok(Reduction::pure(Product(Metadata::new(), factors)))
}

/// Converts a minus to a sum
///
/// ```text
/// x - y ~> x + -y
/// ```
/// does not apply to sets.
/// TODO: need rule to define set difference as a special case of minus, comprehensions needed
/// return type and domain of minus need to be altered too, see expressions.rs
#[register_rule(("Base", 8400))]
fn minus_to_sum(expr: &Expr, _: &SymbolTable) -> ApplicationResult {
    let (lhs, rhs) = match expr {
        Minus(_, lhs, rhs) => {
            if let Some(Set(_)) = lhs.as_ref().return_type() {
                return Err(RuleNotApplicable);
            }
            if let Some(Set(_)) = rhs.as_ref().return_type() {
                return Err(RuleNotApplicable);
            }
            (lhs.clone(), rhs.clone())
        }
        _ => return Err(RuleNotApplicable),
    };

    Ok(Reduction::pure(essence_expr!(&lhs + (-&rhs))))
}
