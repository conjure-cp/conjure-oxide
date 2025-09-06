use conjure_cp::{
    ast::Metadata,
    ast::{
        Atom, DeclarationKind, Expression, Literal, Moo, Name, ReturnType, SymbolTable, Typeable,
    },
    into_matrix_expr, matrix_expr,
    rule_engine::{
        ApplicationError::{self, RuleNotApplicable},
        ApplicationResult, Reduction, register_rule, register_rule_set,
    },
};
use uniplate::{Biplate, Uniplate};

use super::utils::is_all_constant;

register_rule_set!("Bubble", ("Base"));

// Bubble reduction rules

/*
    Reduce bubbles with a boolean expression to a conjunction with their condition.

    e.g. (a / b = c) @ (b != 0) => (a / b = c) & (b != 0)
*/
#[register_rule(("Bubble", 8900))]
fn expand_bubble(expr: &Expression, _: &SymbolTable) -> ApplicationResult {
    match expr {
        Expression::Bubble(_, a, b) if a.return_type() == Some(ReturnType::Bool) => {
            let a = Moo::unwrap_or_clone(Moo::clone(a));
            let b = Moo::unwrap_or_clone(Moo::clone(b));
            Ok(Reduction::pure(Expression::And(
                Metadata::new(),
                Moo::new(matrix_expr![a, b]),
            )))
        }
        _ => Err(ApplicationError::RuleNotApplicable),
    }
}

/*
    Bring bubbles with a non-boolean expression higher up the tree.

    E.g. ((a / b) @ (b != 0)) = c => (a / b = c) @ (b != 0)
*/
#[register_rule(("Bubble", 8800))]
fn bubble_up(expr: &Expression, syms: &SymbolTable) -> ApplicationResult {
    // do not put root inside a bubble
    //
    // also do not bubble bubbles inside bubbles, as this does nothing productive it just shuffles
    // the conditions around, shuffles them back, then gets stuck in a loop doing this adfinitum
    if matches!(expr, Expression::Root(_, _) | Expression::Bubble(_, _, _)) {
        return Err(RuleNotApplicable);
    }

    // do not bubble things containing lettings
    if expr.universe_bi().iter().any(|x: &Name| {
        syms.lookup(x).is_some_and(|x| {
            matches!(
                &x.kind() as &DeclarationKind,
                DeclarationKind::ValueLetting(_)
            )
        })
    }) {
        return Err(RuleNotApplicable);
    };

    let mut sub = expr.children();
    let mut bubbled_conditions = vec![];
    for e in sub.iter_mut() {
        if let Expression::Bubble(_, a, b) = e {
            if a.return_type() != Some(ReturnType::Bool) {
                let a = Moo::unwrap_or_clone(Moo::clone(a));
                let b = Moo::unwrap_or_clone(Moo::clone(b));
                bubbled_conditions.push(b);
                *e = a;
            }
        }
    }
    if bubbled_conditions.is_empty() {
        Err(ApplicationError::RuleNotApplicable)
    } else if bubbled_conditions.len() == 1 {
        let new_expr = Expression::Bubble(
            Metadata::new(),
            Moo::new(expr.with_children(sub)),
            Moo::new(bubbled_conditions[0].clone()),
        );

        Ok(Reduction::pure(new_expr))
    } else {
        Ok(Reduction::pure(Expression::Bubble(
            Metadata::new(),
            Moo::new(expr.with_children(sub)),
            Moo::new(Expression::And(
                Metadata::new(),
                Moo::new(into_matrix_expr![bubbled_conditions]),
            )),
        )))
    }
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
fn div_to_bubble(expr: &Expression, _: &SymbolTable) -> ApplicationResult {
    if is_all_constant(expr) {
        return Err(RuleNotApplicable);
    }
    if let Expression::UnsafeDiv(_, a, b) = expr {
        // bubble bottom up
        if !a.is_safe() || !b.is_safe() {
            return Err(RuleNotApplicable);
        }

        return Ok(Reduction::pure(Expression::Bubble(
            Metadata::new(),
            Moo::new(Expression::SafeDiv(Metadata::new(), a.clone(), b.clone())),
            Moo::new(Expression::Neq(
                Metadata::new(),
                b.clone(),
                Moo::new(Expression::from(0)),
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
fn mod_to_bubble(expr: &Expression, _: &SymbolTable) -> ApplicationResult {
    if is_all_constant(expr) {
        return Err(RuleNotApplicable);
    }
    if let Expression::UnsafeMod(_, a, b) = expr {
        // bubble bottom up
        if !a.is_safe() || !b.is_safe() {
            return Err(RuleNotApplicable);
        }

        return Ok(Reduction::pure(Expression::Bubble(
            Metadata::new(),
            Moo::new(Expression::SafeMod(Metadata::new(), a.clone(), b.clone())),
            Moo::new(Expression::Neq(
                Metadata::new(),
                b.clone(),
                Moo::new(Expression::from(0)),
            )),
        )));
    }
    Err(ApplicationError::RuleNotApplicable)
}

/// Converts an unsafe pow to a safe pow with a bubble condition.
///
/// ```text
/// a**b => (a ** b) @ ((a!=0 \/ b!=0) /\ b>=0
/// ```
///
/// Pow is only defined when `(a!=0 \/ b!=0) /\ b>=0`, so we add a condition to check for it.
/// This condition is brought up the tree and expanded into a conjunction with the first
/// boolean-type expression it is paired with.
///
#[register_rule(("Bubble", 6000))]
fn pow_to_bubble(expr: &Expression, _: &SymbolTable) -> ApplicationResult {
    if is_all_constant(expr) {
        return Err(RuleNotApplicable);
    }
    if let Expression::UnsafePow(_, a, b) = expr.clone() {
        // bubble bottom up
        if !a.is_safe() || !b.is_safe() {
            return Err(RuleNotApplicable);
        }

        return Ok(Reduction::pure(Expression::Bubble(
            Metadata::new(),
            Moo::new(Expression::SafePow(Metadata::new(), a.clone(), b.clone())),
            Moo::new(Expression::And(
                Metadata::new(),
                Moo::new(matrix_expr![
                    Expression::Or(
                        Metadata::new(),
                        Moo::new(matrix_expr![
                            Expression::Neq(
                                Metadata::new(),
                                a,
                                Moo::new(Atom::Literal(Literal::Int(0)).into()),
                            ),
                            Expression::Neq(
                                Metadata::new(),
                                b.clone(),
                                Moo::new(Atom::Literal(Literal::Int(0)).into()),
                            ),
                        ]),
                    ),
                    Expression::Geq(
                        Metadata::new(),
                        b,
                        Moo::new(Atom::Literal(Literal::Int(0)).into()),
                    ),
                ]),
            )),
        )));
    }
    Err(ApplicationError::RuleNotApplicable)
}
