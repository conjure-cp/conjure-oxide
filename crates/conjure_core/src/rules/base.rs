use conjure_core::ast::{Atom, DecisionVariable, Expression as Expr, Literal as Lit, SymbolTable};
use conjure_core::metadata::Metadata;
use conjure_core::rule_engine::{
    register_rule, register_rule_set, ApplicationError, ApplicationError::RuleNotApplicable,
    ApplicationResult, Reduction,
};
use conjure_core::Model;
use uniplate::Uniplate;

use Atom::*;
use Expr::*;
use Lit::*;

/*****************************************************************************/
/*        This file contains basic rules for simplifying expressions         */
/*****************************************************************************/

register_rule_set!("Base", 100, ());

/// This rule simplifies expressions where the operator is applied to an empty set of sub-expressions.
///
/// For example:
/// - `or([])` simplifies to `false` since no disjunction exists.
///
/// **Applicable examples:**
/// ```text
/// or([])  ~> false
/// X([]) ~> Nothing
/// ```
#[register_rule(("Base", 8800))]
fn remove_empty_expression(expr: &Expr, _: &Model) -> ApplicationResult {
    // excluded expressions
    if matches!(
        expr,
        Atomic(_, _)
            | WatchedLiteral(_, _, _)
            | DivEqUndefZero(_, _, _, _)
            | ModuloEqUndefZero(_, _, _, _)
    ) {
        return Err(ApplicationError::RuleNotApplicable);
    }

    if !expr.children().is_empty() {
        return Err(ApplicationError::RuleNotApplicable);
    }

    let new_expr = match expr {
        Or(_, _) => Atomic(Metadata::new(), Literal(Bool(false))),
        _ => And(Metadata::new(), vec![]), // TODO: (yb33) Change it to a simple vector after we refactor our model,
    };

    Ok(Reduction::pure(new_expr))
}

/**
 * Unwrap trivial sums:
 * ```text
 * sum([a]) = a
 * ```
 */
#[register_rule(("Base", 8800))]
fn unwrap_sum(expr: &Expr, _: &Model) -> ApplicationResult {
    match expr {
        Sum(_, exprs) if (exprs.len() == 1) => Ok(Reduction::pure(exprs[0].clone())),
        _ => Err(ApplicationError::RuleNotApplicable),
    }
}

/**
 * Flatten nested sums:
 * ```text
 * sum(sum(a, b), c) = sum(a, b, c)
 * ```
 */
#[register_rule(("Base", 8800))]
pub fn flatten_nested_sum(expr: &Expr, _: &Model) -> ApplicationResult {
    match expr {
        Sum(metadata, exprs) => {
            let mut new_exprs = Vec::new();
            let mut changed = false;
            for e in exprs {
                match e {
                    Sum(_, sub_exprs) => {
                        changed = true;
                        for e in sub_exprs {
                            new_exprs.push(e.clone());
                        }
                    }
                    _ => new_exprs.push(e.clone()),
                }
            }
            if !changed {
                return Err(ApplicationError::RuleNotApplicable);
            }
            Ok(Reduction::pure(Sum(metadata.clone_dirty(), new_exprs)))
        }
        _ => Err(ApplicationError::RuleNotApplicable),
    }
}

/**
* Unwrap nested `or`

* ```text
* or(or(a, b), c) = or(a, b, c)
* ```
 */
#[register_rule(("Base", 8800))]
fn unwrap_nested_or(expr: &Expr, _: &Model) -> ApplicationResult {
    match expr {
        Or(metadata, exprs) => {
            let mut new_exprs = Vec::new();
            let mut changed = false;
            for e in exprs {
                match e {
                    Or(_, exprs) => {
                        changed = true;
                        for e in exprs {
                            new_exprs.push(e.clone());
                        }
                    }
                    _ => new_exprs.push(e.clone()),
                }
            }
            if !changed {
                return Err(ApplicationError::RuleNotApplicable);
            }
            Ok(Reduction::pure(Or(metadata.clone_dirty(), new_exprs)))
        }
        _ => Err(ApplicationError::RuleNotApplicable),
    }
}

/**
* Unwrap nested `and`

* ```text
* and(and(a, b), c) = and(a, b, c)
* ```
 */
#[register_rule(("Base", 8800))]
fn unwrap_nested_and(expr: &Expr, _: &Model) -> ApplicationResult {
    match expr {
        And(metadata, exprs) => {
            let mut new_exprs = Vec::new();
            let mut changed = false;
            for e in exprs {
                match e {
                    And(_, exprs) => {
                        changed = true;
                        for e in exprs {
                            new_exprs.push(e.clone());
                        }
                    }
                    _ => new_exprs.push(e.clone()),
                }
            }
            if !changed {
                return Err(ApplicationError::RuleNotApplicable);
            }
            Ok(Reduction::pure(And(metadata.clone_dirty(), new_exprs)))
        }
        _ => Err(ApplicationError::RuleNotApplicable),
    }
}

/**
* Remove double negation:

* ```text
* not(not(a)) = a
* ```
 */
#[register_rule(("Base", 8400))]
fn remove_double_negation(expr: &Expr, _: &Model) -> ApplicationResult {
    match expr {
        Not(_, contents) => match contents.as_ref() {
            Not(_, expr_box) => Ok(Reduction::pure(*expr_box.clone())),
            _ => Err(ApplicationError::RuleNotApplicable),
        },
        _ => Err(ApplicationError::RuleNotApplicable),
    }
}

/**
 * Remove trivial `and` (only one element):
 * ```text
 * and([a]) = a
 * ```
 */
#[register_rule(("Base", 8800))]
fn remove_trivial_and(expr: &Expr, _: &Model) -> ApplicationResult {
    match expr {
        And(_, exprs) => {
            if exprs.len() == 1 {
                return Ok(Reduction::pure(exprs[0].clone()));
            }
            Err(ApplicationError::RuleNotApplicable)
        }
        _ => Err(ApplicationError::RuleNotApplicable),
    }
}

/**
 * Remove trivial `or` (only one element):
 * ```text
 * or([a]) = a
 * ```
 */
#[register_rule(("Base", 8800))]
fn remove_trivial_or(expr: &Expr, _: &Model) -> ApplicationResult {
    match expr {
        // do not conflict with unwrap_nested_or rule.
        Or(_, exprs) if exprs.len() == 1 && !matches!(exprs[0], Or(_, _)) => {
            Ok(Reduction::pure(exprs[0].clone()))
        }
        _ => Err(ApplicationError::RuleNotApplicable),
    }
}

/**
<<<<<<< HEAD
 * Remove constant bools from or expressions
 * ```text
 * or([true, a]) = true
 * or([false, a]) = a
 * ```
 */
#[register_rule(("Base", 100))]
fn remove_constants_from_or(expr: &Expr, _: &Model) -> ApplicationResult {
    match expr {
        Or(metadata, exprs) => {
            let mut new_exprs = Vec::new();
            let mut changed = false;
            for e in exprs {
                match e {
                    Atomic(metadata, Literal(Bool(val))) => {
                        if *val {
                            // If we find a true, the whole expression is true
                            return Ok(Reduction::pure(Atomic(
                                metadata.clone_dirty(),
                                Literal(Bool(true)),
                            )));
                        } else {
                            // If we find a false, we can ignore it
                            changed = true;
                        }
                    }
                    _ => new_exprs.push(e.clone()),
                }
            }
            if !changed {
                return Err(ApplicationError::RuleNotApplicable);
            }
            Ok(Reduction::pure(Or(metadata.clone_dirty(), new_exprs)))
        }
        _ => Err(ApplicationError::RuleNotApplicable),
    }
}

/**
 * Remove constant bools from and expressions
 * ```text
 * and([true, a]) = a
 * and([false, a]) = false
 * ```
 */
#[register_rule(("Base", 100))]
fn remove_constants_from_and(expr: &Expr, _: &Model) -> ApplicationResult {
    match expr {
        And(metadata, exprs) => {
            let mut new_exprs = Vec::new();
            let mut changed = false;
            for e in exprs {
                match e {
                    Atomic(metadata, Literal(Bool(val))) => {
                        if !*val {
                            // If we find a false, the whole expression is false
                            return Ok(Reduction::pure(Atomic(
                                metadata.clone_dirty(),
                                Literal(Bool(false)),
                            )));
                        } else {
                            // If we find a true, we can ignore it
                            changed = true;
                        }
                    }
                    _ => new_exprs.push(e.clone()),
                }
            }
            if !changed {
                return Err(ApplicationError::RuleNotApplicable);
            }
            Ok(Reduction::pure(And(metadata.clone_dirty(), new_exprs)))
        }
        _ => Err(ApplicationError::RuleNotApplicable),
    }
}

/**
 * Evaluate Not expressions with constant bools
 * ```text
 * not(true) = false
 * not(false) = true
 * ```
 */
#[register_rule(("Base", 100))]
fn evaluate_constant_not(expr: &Expr, _: &Model) -> ApplicationResult {
    match expr {
        Not(_, contents) => match contents.as_ref() {
            Atomic(metadata, Literal(Bool(val))) => Ok(Reduction::pure(Atomic(
                metadata.clone_dirty(),
                Literal(Bool(!val)),
            ))),
            _ => Err(ApplicationError::RuleNotApplicable),
        },
        _ => Err(ApplicationError::RuleNotApplicable),
    }
}

/**
 * Turn a Min into a new variable and post a top-level constraint to ensure the new variable is the minimum.
 * ```text
 * min([a, b]) ~> c ; c <= a & c <= b & (c = a | c = b)
 * ```
 */
#[register_rule(("Base", 2000))]
fn min_to_var(expr: &Expr, mdl: &Model) -> ApplicationResult {
    match expr {
        Min(metadata, exprs) => {
            let new_name = mdl.gensym();

            let mut new_top = Vec::new(); // the new variable must be less than or equal to all the other variables
            let mut disjunction = Vec::new(); // the new variable must be equal to one of the variables
            for e in exprs {
                new_top.push(Leq(
                    Metadata::new(),
                    Box::new(Atomic(Metadata::new(), Reference(new_name.clone()))),
                    Box::new(e.clone()),
                ));
                disjunction.push(Eq(
                    Metadata::new(),
                    Box::new(Atomic(Metadata::new(), Reference(new_name.clone()))),
                    Box::new(e.clone()),
                ));
            }
            new_top.push(Or(Metadata::new(), disjunction));

            let mut new_vars = SymbolTable::new();
            let domain = expr
                .domain_of(&mdl.variables)
                .ok_or(ApplicationError::DomainError)?;
            new_vars.insert(new_name.clone(), DecisionVariable::new(domain));

            Ok(Reduction::new(
                Atomic(Metadata::new(), Reference(new_name)),
                And(metadata.clone_dirty(), new_top),
                new_vars,
            ))
        }
        _ => Err(ApplicationError::RuleNotApplicable),
    }
}

/**
 * Turn a Max into a new variable and post a top level constraint to ensure the new variable is the maximum.
 * ```text
 * max([a, b]) ~> c ; c >= a & c >= b & (c = a | c = b)
 * ```
 */
#[register_rule(("Base", 100))]
fn max_to_var(expr: &Expr, mdl: &Model) -> ApplicationResult {
    match expr {
        Max(metadata, exprs) => {
            let new_name = mdl.gensym();

            let mut new_top = Vec::new(); // the new variable must be more than or equal to all the other variables
            let mut disjunction = Vec::new(); // the new variable must more than or equal to one of the variables
            for e in exprs {
                new_top.push(Geq(
                    Metadata::new(),
                    Box::new(Atomic(Metadata::new(), Reference(new_name.clone()))),
                    Box::new(e.clone()),
                ));
                disjunction.push(Eq(
                    Metadata::new(),
                    Box::new(Atomic(Metadata::new(), Reference(new_name.clone()))),
                    Box::new(e.clone()),
                ));
            }
            new_top.push(Or(Metadata::new(), disjunction));

            let mut new_vars = SymbolTable::new();
            let domain = expr
                .domain_of(&mdl.variables)
                .ok_or(ApplicationError::DomainError)?;
            new_vars.insert(new_name.clone(), DecisionVariable::new(domain));

            Ok(Reduction::new(
                Atomic(Metadata::new(), Reference(new_name)),
                And(metadata.clone_dirty(), new_top),
                new_vars,
            ))
        }
        _ => Err(ApplicationError::RuleNotApplicable),
    }
}

/**
* Apply the Distributive Law to expressions like `Or([..., And(a, b)])`

* ```text
* or(and(a, b), c) = and(or(a, c), or(b, c))
* ```
 */
#[register_rule(("Base", 8400))]
fn distribute_or_over_and(expr: &Expr, _: &Model) -> ApplicationResult {
    fn find_and(exprs: &[Expr]) -> Option<usize> {
        // ToDo: may be better to move this to some kind of utils module?
        for (i, e) in exprs.iter().enumerate() {
            if let And(_, _) = e {
                return Some(i);
            }
        }
        None
    }

    match expr {
        Or(_, exprs) => match find_and(exprs) {
            Some(idx) => {
                let mut rest = exprs.clone();
                let and_expr = rest.remove(idx);

                match and_expr {
                    And(metadata, and_exprs) => {
                        let mut new_and_contents = Vec::new();

                        for e in and_exprs {
                            // ToDo: Cloning everything may be a bit inefficient - discuss
                            let mut new_or_contents = rest.clone();
                            new_or_contents.push(e.clone());
                            new_and_contents.push(Or(metadata.clone_dirty(), new_or_contents))
                        }

                        Ok(Reduction::pure(And(
                            metadata.clone_dirty(),
                            new_and_contents,
                        )))
                    }
                    _ => Err(ApplicationError::RuleNotApplicable),
                }
            }
            None => Err(ApplicationError::RuleNotApplicable),
        },
        _ => Err(ApplicationError::RuleNotApplicable),
    }
}

/**
* Distribute `not` over `and` (De Morgan's Law):

* ```text
* not(and(a, b)) = or(not a, not b)
* ```
 */
#[register_rule(("Base", 8400))]
fn distribute_not_over_and(expr: &Expr, _: &Model) -> ApplicationResult {
    for child in expr.universe() {
        if matches!(
            child,
            Expr::UnsafeDiv(_, _, _) | Expr::Bubble(_, _, _) | Expr::UnsafeMod(_, _, _)
        ) {
            return Err(RuleNotApplicable);
        }
    }
    match expr {
        Not(_, contents) => match contents.as_ref() {
            And(metadata, exprs) => {
                if exprs.len() == 1 {
                    let single_expr = exprs[0].clone();
                    return Ok(Reduction::pure(Not(
                        Metadata::new(),
                        Box::new(single_expr.clone()),
                    )));
                }
                let mut new_exprs = Vec::new();
                for e in exprs {
                    new_exprs.push(Not(metadata.clone(), Box::new(e.clone())));
                }
                Ok(Reduction::pure(Or(metadata.clone(), new_exprs)))
            }
            _ => Err(ApplicationError::RuleNotApplicable),
        },
        _ => Err(ApplicationError::RuleNotApplicable),
    }
}

/**
* Distribute `not` over `or` (De Morgan's Law):

* ```text
* not(or(a, b)) = and(not a, not b)
* ```
 */
#[register_rule(("Base", 8400))]
fn distribute_not_over_or(expr: &Expr, _: &Model) -> ApplicationResult {
    match expr {
        Not(_, contents) => match contents.as_ref() {
            Or(metadata, exprs) => {
                if exprs.len() == 1 {
                    let single_expr = exprs[0].clone();
                    return Ok(Reduction::pure(Not(
                        Metadata::new(),
                        Box::new(single_expr.clone()),
                    )));
                }
                let mut new_exprs = Vec::new();
                for e in exprs {
                    new_exprs.push(Not(metadata.clone(), Box::new(e.clone())));
                }
                Ok(Reduction::pure(And(metadata.clone(), new_exprs)))
            }
            _ => Err(ApplicationError::RuleNotApplicable),
        },
        _ => Err(ApplicationError::RuleNotApplicable),
    }
}

#[register_rule(("Base", 8800))]
fn negated_neq_to_eq(expr: &Expr, _: &Model) -> ApplicationResult {
    match expr {
        Not(_, a) => match a.as_ref() {
            Neq(_, b, c) if (!b.can_be_undefined() && !c.can_be_undefined()) => {
                Ok(Reduction::pure(Eq(Metadata::new(), b.clone(), c.clone())))
            }
            _ => Err(RuleNotApplicable),
        },
        _ => Err(RuleNotApplicable),
    }
}

#[register_rule(("Base", 8800))]
fn negated_eq_to_neq(expr: &Expr, _: &Model) -> ApplicationResult {
    match expr {
        Not(_, a) => match a.as_ref() {
            Eq(_, b, c) if (!b.can_be_undefined() && !c.can_be_undefined()) => {
                Ok(Reduction::pure(Neq(Metadata::new(), b.clone(), c.clone())))
            }
            _ => Err(RuleNotApplicable),
        },
        _ => Err(RuleNotApplicable),
    }
}
