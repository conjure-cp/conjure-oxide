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
    ast::{AbstractLiteral, Atom, Expression as Expr, Literal as Lit, SymbolTable},
    matrix_expr,
    metadata::Metadata,
    rule_engine::{
        register_rule, ApplicationError::RuleNotApplicable, ApplicationResult, Reduction,
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
            Ok(Reduction::pure(a.children()[0].clone()))
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
        Expr::Neg(_, e) if matches!(**e, Expr::Sum(_, _)) => Ok(*e.clone()),
        _ => Err(RuleNotApplicable),
    }?;

    let mut child_vecs: VecDeque<Vec<Expr>> = inner_expr.children_bi();

    if child_vecs.is_empty() {
        return Err(RuleNotApplicable);
    }

    for child in child_vecs[0].iter_mut() {
        *child = Expr::Neg(Metadata::new(), Box::new(child.clone()))
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
    let Expr::Neg(_, expr1) = expr.clone() else {
        return Err(RuleNotApplicable);
    };

    let Expr::Product(_, mut factors) = *expr1 else {
        return Err(RuleNotApplicable);
    };

    factors.push(Expr::Atomic(Metadata::new(), Atom::Literal(Lit::Int(-1))));

    Ok(Reduction::pure(Expr::Product(Metadata::new(), factors)))
}

/// Converts a minus to a sum
///
/// ```text
/// x - y ~> x + -y
/// ```
#[register_rule(("Base", 8400))]
fn minus_to_sum(expr: &Expr, _: &SymbolTable) -> ApplicationResult {
    let (lhs, rhs) = match expr {
        Expr::Minus(_, lhs, rhs) => {
            // println!("lhs:");
            // print!("{:?}", lhs);
            // println!("rhs:");
            // print!("{:?}", rhs);
            match lhs.as_ref() {
                Expr::Atomic(_, Atom::Reference(name)) => match rhs.as_ref() {
                    Expr::Atomic(_, Atom::Reference(name)) => {
                        return Err(RuleNotApplicable);
                    }
                    Expr::AbstractLiteral(_, c1) => match c1 {
                        AbstractLiteral::Set(t1) => {
                            return Err(RuleNotApplicable);
                        }
                        _ => (lhs.clone(), rhs.clone()),
                    },
                    _ => (lhs.clone(), rhs.clone()),
                },
                Expr::AbstractLiteral(_, c1) => match c1 {
                    AbstractLiteral::Set(t1) => match rhs.as_ref() {
                        Expr::Atomic(_, Atom::Reference(name)) => {
                            return Err(RuleNotApplicable);
                        }
                        Expr::AbstractLiteral(_, c1) => match c1 {
                            AbstractLiteral::Set(t1) => {
                                return Err(RuleNotApplicable);
                            }
                            _ => (lhs.clone(), rhs.clone()),
                        },
                        _ => (lhs.clone(), rhs.clone()),
                    },
                    _ => (lhs.clone(), rhs.clone()),
                },
                _ => (lhs.clone(), rhs.clone()),
            }
        }
        _ => return Err(RuleNotApplicable),
    };

    Ok(Reduction::pure(Expr::Sum(
        Metadata::new(),
        Box::new(matrix_expr![*lhs, Expr::Neg(Metadata::new(), rhs)]),
    )))
}

#[register_rule(("Base", 8500))]
fn minus_sets(expr: &Expr, _: &SymbolTable) -> ApplicationResult {
    let (lhs, rhs) = match expr {
        Expr::Minus(_, lhs, rhs) => match lhs.as_ref() {
            Expr::Atomic(_, Atom::Reference(name)) => match rhs.as_ref() {
                Expr::Atomic(_, Atom::Reference(name)) => {
                    println!("lhs expr, rhs expr");
                    print!("{:?}", lhs);
                    print!("{:?}", rhs);
                    (lhs.clone(), rhs.clone())
                }
                Expr::AbstractLiteral(_, c1) => match c1 {
                    AbstractLiteral::Set(t1) => {
                        println!("lhs expr, rhs abstract lit");
                        print!("{:?}", lhs);
                        print!("{:?}", rhs);
                        (lhs.clone(), rhs.clone())
                    }
                    _ => return Err(RuleNotApplicable),
                },
                _ => return Err(RuleNotApplicable),
            },
            Expr::AbstractLiteral(_, c1) => match c1 {
                AbstractLiteral::Set(t1) => match rhs.as_ref() {
                    Expr::Atomic(_, Atom::Reference(name)) => {
                        println!("lhs abstract lit, rhs expr");
                        print!("{:?}", lhs);
                        print!("{:?}", rhs);
                        (lhs.clone(), rhs.clone())
                    }
                    Expr::AbstractLiteral(_, c1) => match c1 {
                        AbstractLiteral::Set(t1) => {
                            println!("lhs abstract lit, rhs abstract lit");
                            print!("{:?}", lhs);
                            print!("{:?}", rhs);
                            (lhs.clone(), rhs.clone())
                        }
                        _ => return Err(RuleNotApplicable),
                    },
                    _ => return Err(RuleNotApplicable),
                },
                _ => return Err(RuleNotApplicable),
            },
            _ => return Err(RuleNotApplicable),
        },
        _ => return Err(RuleNotApplicable),
    };

    return Err(RuleNotApplicable);
}
