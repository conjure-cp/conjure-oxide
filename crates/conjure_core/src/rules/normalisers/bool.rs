//! Normalising rules for boolean operations (not, and, or).

use conjure_core::ast::Expression as Expr;
use conjure_core::metadata::Metadata;
use conjure_core::rule_engine::{
    register_rule, ApplicationError, ApplicationError::RuleNotApplicable, ApplicationResult,
    Reduction,
};
use conjure_core::Model;
use uniplate::Uniplate;

use Expr::*;

/// Removes double negations
///
/// ```text
/// not(not(a)) = a
/// ```
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

/// Distributes `ands` contained in `ors`
///
/// ```text
/// or(and(a, b), c) ~> and(or(a, c), or(b, c))
/// ```
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

/// Distributes `not` over `and` by De Morgan's Law
///
/// ```text
/// not(and(a, b)) ~> or(not(a), not(b))
/// ```
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

/// Distributes `not` over `or` by De Morgan's Law
///
/// ```text
/// not(or(a, b)) ~> and(not(a), not(b))
/// ```
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

/// Unwraps nested `or`
///
/// ```text
/// or(or(a, b), c) ~> or(a, b, c)
/// ```
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

/// Unwraps nested `and`
///
/// ``text
/// and(and(a, b), c) ~> and(a, b, c)
/// ```
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

/// Removes ands with a single argument.
///
/// ```text
/// or([a]) ~> a
/// ```
#[register_rule(("Base", 8800))]
fn remove_unit_vector_and(expr: &Expr, _: &Model) -> ApplicationResult {
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

/// Removes ors with a single argument.
///
/// ```text
/// or([a]) ~> a
/// ```
#[register_rule(("Base", 8800))]
fn remove_unit_vector_or(expr: &Expr, _: &Model) -> ApplicationResult {
    match expr {
        // do not conflict with unwrap_nested_or rule.
        Or(_, exprs) if exprs.len() == 1 && !matches!(exprs[0], Or(_, _)) => {
            Ok(Reduction::pure(exprs[0].clone()))
        }
        _ => Err(ApplicationError::RuleNotApplicable),
    }
}
