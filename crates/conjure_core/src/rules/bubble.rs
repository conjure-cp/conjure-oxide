use conjure_core::ast::{Expression, ReturnType};
use conjure_core::metadata::Metadata;
use conjure_core::rule_engine::{
    register_rule, register_rule_set, ApplicationError, ApplicationError::*, ApplicationResult,
    Reduction,
};
use conjure_core::Model;
use uniplate::Uniplate;

use super::utils::is_all_constant;

register_rule_set!("Bubble", 100, ("Base"));

// Bubble reduction rules

/*
    Reduce bubbles with a boolean expression to a conjunction with their condition.

    e.g. (a / b = c) @ (b != 0) => (a / b = c) & (b != 0)
*/
#[register_rule(("Bubble", 8900))]
fn expand_bubble(expr: &Expression, _: &Model) -> ApplicationResult {
    match expr {
        Expression::Bubble(_, a, b) if a.return_type() == Some(ReturnType::Bool) => {
            Ok(Reduction::pure(Expression::And(
                Metadata::new(),
                vec![*a.clone(), *b.clone()],
            )))
        }
        _ => Err(ApplicationError::RuleNotApplicable),
    }
}

/*
    Bring bubbles with a non-boolean expression higher up the tree.

    E.g. ((a / b) @ (b != 0)) = c => (a / b = c) @ (b != 0)
*/
#[register_rule(("Bubble", 8900))]
fn bubble_up(expr: &Expression, _: &Model) -> ApplicationResult {
    let mut sub = expr.children();
    let mut bubbled_conditions = vec![];
    for e in sub.iter_mut() {
        if let Expression::Bubble(_, a, b) = e {
            if a.return_type() != Some(ReturnType::Bool) {
                bubbled_conditions.push(*b.clone());
                *e = *a.clone();
            }
        }
    }
    if bubbled_conditions.is_empty() {
        return Err(ApplicationError::RuleNotApplicable);
    }
    Ok(Reduction::pure(Expression::Bubble(
        Metadata::new(),
        Box::new(expr.with_children(sub)),
        Box::new(Expression::And(Metadata::new(), bubbled_conditions)),
    )))
}

// Bubble applications

/// Converts an unsafe division to a safe division with a bubble condition.
///
/// ```text
///     a / b => (a / b) @ (b != 0)
/// ```
///
/// Division by zero is undefined and therefore not allowed, so we add a condition to check for it.
/// This condition is brought up the tree and expanded into a conjunction with the first
/// boolean-type expression it is paired with.

#[register_rule(("Bubble", 6000))]
fn div_to_bubble(expr: &Expression, _: &Model) -> ApplicationResult {
    if is_all_constant(expr) {
        return Err(RuleNotApplicable);
    }
    if let Expression::UnsafeDiv(_, a, b) = expr {
        // bubble bottom up
        if a.can_be_undefined() || b.can_be_undefined() {
            return Err(RuleNotApplicable);
        }

        // either do bubble / bubble or not bubble / not bubble
        if matches!(**a, Expression::Bubble(_, _, _)) != matches!(**b, Expression::Bubble(_, _, _))
        {
            return Err(RuleNotApplicable);
        }

        return Ok(Reduction::pure(Expression::Bubble(
            Metadata::new(),
            Box::new(Expression::SafeDiv(Metadata::new(), a.clone(), b.clone())),
            Box::new(Expression::Neq(
                Metadata::new(),
                b.clone(),
                Box::new(Expression::from(0)),
            )),
        )));
    }
    Err(ApplicationError::RuleNotApplicable)
}

/// Converts an unsafe mod to a safe mod with a bubble condition.
///
/// ```text
/// a % b => (a % b) @ (b != 0)
/// ```
///
/// Mod zero is undefined and therefore not allowed, so we add a condition to check for it.
/// This condition is brought up the tree and expanded into a conjunction with the first
/// boolean-type expression it is paired with.
///
#[register_rule(("Bubble", 6000))]
fn mod_to_bubble(expr: &Expression, _: &Model) -> ApplicationResult {
    if is_all_constant(expr) {
        return Err(RuleNotApplicable);
    }
    if let Expression::UnsafeMod(_, a, b) = expr {
        // bubble bottom up
        if a.can_be_undefined() || b.can_be_undefined() {
            return Err(RuleNotApplicable);
        }

        // either do bubble / bubble or not bubble / not bubble
        if matches!(**a, Expression::Bubble(_, _, _)) != matches!(**b, Expression::Bubble(_, _, _))
        {
            return Err(RuleNotApplicable);
        }

        return Ok(Reduction::pure(Expression::Bubble(
            Metadata::new(),
            Box::new(Expression::SafeMod(Metadata::new(), a.clone(), b.clone())),
            Box::new(Expression::Neq(
                Metadata::new(),
                b.clone(),
                Box::new(Expression::from(0)),
            )),
        )));
    }
    Err(ApplicationError::RuleNotApplicable)
}
