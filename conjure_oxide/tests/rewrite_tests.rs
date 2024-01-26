// Tests for rewriting/simplifying parts of the AST

use core::panic;
use std::collections::HashMap;

use conjure_oxide::{
    ast::*,
    solvers::{FromConjureModel},
};
use conjure_rules::{get_rule_by_name};
use minion_rs::ast::{Constant, VarName};

#[test]
fn rules_present() {
    let rules = conjure_rules::get_rules();
    assert!(!rules.is_empty());
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
        Some(result) => assert_eq!(result, 6),
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
    assert_eq!(simplified_expression, correct_simplified_expression);
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

fn callback(solution: HashMap<VarName, Constant>) -> bool {
    println!("Solution: {:?}", solution);
    false
}

///
/// Reduce and solve:
/// ```text
/// find a,b,c : int(1..3)
/// such that a + b + c <= 2 + 3 - 1
/// such that a < b
/// ```
#[test]
fn reduce_solve_xyz() {
    println!("Rules: {:?}", conjure_rules::get_rules());
    let sum_constants = get_rule_by_name("sum_constants").unwrap();
    let unwrap_sum = get_rule_by_name("unwrap_sum").unwrap();
    let lt_to_ineq = get_rule_by_name("lt_to_ineq").unwrap();
    let sum_leq_to_sumleq = get_rule_by_name("sum_leq_to_sumleq").unwrap();

    // 2 + 3 - 1
    let mut expr1 = Expression::Sum(vec![
        Expression::ConstantInt(2),
        Expression::ConstantInt(3),
        Expression::ConstantInt(-1),
    ]);

    expr1 = sum_constants.apply(&expr1).unwrap();
    expr1 = unwrap_sum.apply(&expr1).unwrap();
    assert_eq!(expr1, Expression::ConstantInt(4));

    // a + b + c = 4
    expr1 = Expression::Leq(
        Box::new(Expression::Sum(vec![
            Expression::Reference(Name::UserName(String::from("a"))),
            Expression::Reference(Name::UserName(String::from("b"))),
            Expression::Reference(Name::UserName(String::from("c"))),
        ])),
        Box::new(expr1),
    );
    expr1 = sum_leq_to_sumleq.apply(&expr1).unwrap();
    assert_eq!(
        expr1,
        Expression::SumLeq(
            vec![
                Expression::Reference(Name::UserName(String::from("a"))),
                Expression::Reference(Name::UserName(String::from("b"))),
                Expression::Reference(Name::UserName(String::from("c"))),
            ],
            Box::new(Expression::ConstantInt(4))
        )
    );

    // a < b
    let mut expr2 = Expression::Lt(
        Box::new(Expression::Reference(Name::UserName(String::from("a")))),
        Box::new(Expression::Reference(Name::UserName(String::from("b")))),
    );
    expr2 = lt_to_ineq.apply(&expr2).unwrap();
    assert_eq!(
        expr2,
        Expression::Ineq(
            Box::new(Expression::Reference(Name::UserName(String::from("a")))),
            Box::new(Expression::Reference(Name::UserName(String::from("b")))),
            Box::new(Expression::ConstantInt(-1))
        )
    );

    let mut model = Model {
        variables: HashMap::new(),
        constraints: vec![expr1, expr2],
    };
    model.variables.insert(
        Name::UserName(String::from("a")),
        DecisionVariable {
            domain: Domain::IntDomain(vec![Range::Bounded(1, 3)]),
        },
    );
    model.variables.insert(
        Name::UserName(String::from("b")),
        DecisionVariable {
            domain: Domain::IntDomain(vec![Range::Bounded(1, 3)]),
        },
    );
    model.variables.insert(
        Name::UserName(String::from("c")),
        DecisionVariable {
            domain: Domain::IntDomain(vec![Range::Bounded(1, 3)]),
        },
    );

    let minion_model = conjure_oxide::solvers::minion::MinionModel::from_conjure(model).unwrap();

    minion_rs::run_minion(minion_model, callback).unwrap();
}
