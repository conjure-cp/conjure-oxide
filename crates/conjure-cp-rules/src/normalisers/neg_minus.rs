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

use conjure_cp::essence_expr;
use conjure_cp::{
    ast::Metadata,
    ast::{Expression as Expr, Moo, ReturnType::Set, SymbolTable, Typeable},
    into_matrix_expr,
    rule_engine::{
        ApplicationError::RuleNotApplicable, ApplicationResult, Reduction, register_rule,
    },
};
use std::collections::VecDeque;
use uniplate::{Biplate, Uniplate as _};

/// Eliminates double negation
///
/// ```text
/// --x ~> x
/// ```
#[register_rule(("Base", 8400))]
fn elmininate_double_negation(expr: &Expr, _: &SymbolTable) -> ApplicationResult {
    match expr {
        Expr::Neg(_, a) if matches!(**a, Expr::Neg(_, _)) => {
            let first_child: Expr = a.as_ref().children()[0].clone();
            Ok(Reduction::pure(first_child))
        }
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
        Expr::Neg(_, e) if matches!(**e, Expr::Sum(_, _)) => Ok(e),
        _ => Err(RuleNotApplicable),
    }?;

    let mut child_vecs: VecDeque<Vec<Expr>> = inner_expr.children_bi();

    if child_vecs.is_empty() {
        return Err(RuleNotApplicable);
    }

    for child in child_vecs[0].iter_mut() {
        *child = essence_expr!(-&child);
    }

    Ok(Reduction::pure(Moo::unwrap_or_clone(
        inner_expr.with_children_bi(child_vecs),
    )))
}

/// Simplifies the negation of a product
///
/// ```text
/// -(x * y) ~> -1 * x * y
/// ```
#[register_rule(("Base", 8400))]
fn simplify_negation_of_product(expr: &Expr, _: &SymbolTable) -> ApplicationResult {
    let Expr::Neg(_, expr1) = expr.clone() else {
        return Err(RuleNotApplicable);
    };

    let Expr::Product(_, factors) = Moo::unwrap_or_clone(expr1) else {
        return Err(RuleNotApplicable);
    };

    let mut factors = Moo::unwrap_or_clone(factors)
        .unwrap_list()
        .ok_or(RuleNotApplicable)?;

    factors.push(essence_expr!(-1));

    Ok(Reduction::pure(Expr::Product(
        Metadata::new(),
        Moo::new(into_matrix_expr!(factors)),
    )))
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
        Expr::Minus(_, lhs, rhs) => {
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
