// Tests for rewriting/simplifying parts of the AST

use core::panic;

use conjure_oxide::ast::*;
use conjure_rules::{get_rule_by_name, get_rules};

#[test]
fn rules_present() {
    let rules = get_rules();
    assert!(rules.len() > 0);
}

#[test]
fn sum_of_constants() {
    let valid_sum_expression = Expression::Sum(vec![
        Expression::ConstantInt(1),
        Expression::ConstantInt(2),
        Expression::ConstantInt(3),
    ]);

    let invalid_sum_expression = Expression::Sum(vec![
        Expression::ConstantInt(1),
        Expression::Reference(Name::UserName(String::from("a"))),
    ]);

    match evaluate_sum_of_constants(&valid_sum_expression) {
        Some(result) => assert!(result == 6),
        None => panic!(),
    }

    match evaluate_sum_of_constants(&invalid_sum_expression) {
        Some(_) => panic!(),
        None => (),
    }
}

fn evaluate_sum_of_constants(expr: &Expression) -> Option<i32> {
    match expr {
        Expression::Sum(expressions) => {
            let mut sum = 0;
            for e in expressions {
                match e {
                    Expression::ConstantInt(value) => {
                        sum += value;
                    }
                    _ => return None,
                }
            }
            Some(sum)
        }
        _ => None,
    }
}

#[test]
fn recursive_sum_of_constants() {
    let complex_expression = Expression::Eq(
        Box::new(Expression::Sum(vec![
            Expression::ConstantInt(1),
            Expression::ConstantInt(2),
            Expression::Sum(vec![Expression::ConstantInt(1), Expression::ConstantInt(2)]),
            Expression::Reference(Name::UserName(String::from("a"))),
        ])),
        Box::new(Expression::ConstantInt(3)),
    );
    let correct_simplified_expression = Expression::Eq(
        Box::new(Expression::Sum(vec![
            Expression::ConstantInt(1),
            Expression::ConstantInt(2),
            Expression::ConstantInt(3),
            Expression::Reference(Name::UserName(String::from("a"))),
        ])),
        Box::new(Expression::ConstantInt(3)),
    );

    let simplified_expression = simplify_expression(complex_expression.clone());
    assert!(simplified_expression == correct_simplified_expression);
}

fn simplify_expression(expr: Expression) -> Expression {
    match expr {
        Expression::Sum(expressions) => {
            if let Some(result) = evaluate_sum_of_constants(&Expression::Sum(expressions.clone())) {
                Expression::ConstantInt(result)
            } else {
                Expression::Sum(expressions.into_iter().map(simplify_expression).collect())
            }
        }
        Expression::Eq(left, right) => Expression::Eq(
            Box::new(simplify_expression(*left)),
            Box::new(simplify_expression(*right)),
        ),
        Expression::Geq(left, right) => Expression::Geq(
            Box::new(simplify_expression(*left)),
            Box::new(simplify_expression(*right)),
        ),
        _ => expr,
    }
}

#[test]
fn rule_sum_constants() {
    let sum_constants = get_rule_by_name("sum_constants").unwrap();
    let unwrap_sum = get_rule_by_name("unwrap_sum").unwrap();

    let mut expr = Expression::Sum(vec![
        Expression::ConstantInt(1),
        Expression::ConstantInt(2),
        Expression::ConstantInt(3),
    ]);

    expr = sum_constants.apply(&expr).unwrap();
    expr = unwrap_sum.apply(&expr).unwrap();

    assert_eq!(expr, Expression::ConstantInt(6));
}

#[test]
fn rule_sum_mixed() {
    let sum_constants = get_rule_by_name("sum_constants").unwrap();

    let mut expr = Expression::Sum(vec![
        Expression::ConstantInt(1),
        Expression::ConstantInt(2),
        Expression::Reference(Name::UserName(String::from("a"))),
    ]);

    expr = sum_constants.apply(&expr).unwrap();

    assert_eq!(
        expr,
        Expression::Sum(vec![
            Expression::Reference(Name::UserName(String::from("a"))),
            Expression::ConstantInt(3),
        ])
    );
}

#[test]
fn rule_sum_geq() {
    let flatten_sum_geq = get_rule_by_name("flatten_sum_geq").unwrap();

    let mut expr = Expression::Geq(
        Box::new(Expression::Sum(vec![
            Expression::ConstantInt(1),
            Expression::ConstantInt(2),
        ])),
        Box::new(Expression::ConstantInt(3)),
    );

    expr = flatten_sum_geq.apply(&expr).unwrap();

    assert_eq!(
        expr,
        Expression::SumGeq(
            vec![Expression::ConstantInt(1), Expression::ConstantInt(2),],
            Box::new(Expression::ConstantInt(3))
        )
    );
}
