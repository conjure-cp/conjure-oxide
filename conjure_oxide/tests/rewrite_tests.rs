// Tests for rewriting/simplifying parts of the AST

use core::panic;
use std::collections::HashMap;

use conjure_oxide::eval_constant;
use conjure_oxide::{ast::*, solvers::FromConjureModel};
use conjure_rules::get_rule_by_name;
use minion_rs::ast::{Constant as MinionConstant, VarName};

#[test]
fn rules_present() {
    let rules = conjure_rules::get_rules();
    assert!(!rules.is_empty());
}

#[test]
fn sum_of_constants() {
    let valid_sum_expression = Expression::Sum(vec![
        Expression::Constant(Constant::Int(1)),
        Expression::Constant(Constant::Int(2)),
        Expression::Constant(Constant::Int(3)),
    ]);

    let invalid_sum_expression = Expression::Sum(vec![
        Expression::Constant(Constant::Int(1)),
        Expression::Reference(Name::UserName(String::from("a"))),
    ]);

    match evaluate_sum_of_constants(&valid_sum_expression) {
        Some(result) => assert_eq!(result, 6),
        None => panic!(),
    }

    if evaluate_sum_of_constants(&invalid_sum_expression).is_some() {
        panic!()
    }
}

fn evaluate_sum_of_constants(expr: &Expression) -> Option<i32> {
    match expr {
        Expression::Sum(expressions) => {
            let mut sum = 0;
            for e in expressions {
                match e {
                    Expression::Constant(Constant::Int(value)) => {
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
            Expression::Constant(Constant::Int(1)),
            Expression::Constant(Constant::Int(2)),
            Expression::Sum(vec![
                Expression::Constant(Constant::Int(1)),
                Expression::Constant(Constant::Int(2)),
            ]),
            Expression::Reference(Name::UserName(String::from("a"))),
        ])),
        Box::new(Expression::Constant(Constant::Int(3))),
    );
    let correct_simplified_expression = Expression::Eq(
        Box::new(Expression::Sum(vec![
            Expression::Constant(Constant::Int(1)),
            Expression::Constant(Constant::Int(2)),
            Expression::Constant(Constant::Int(3)),
            Expression::Reference(Name::UserName(String::from("a"))),
        ])),
        Box::new(Expression::Constant(Constant::Int(3))),
    );

    let simplified_expression = simplify_expression(complex_expression.clone());
    assert_eq!(simplified_expression, correct_simplified_expression);
}

fn simplify_expression(expr: Expression) -> Expression {
    match expr {
        Expression::Sum(expressions) => {
            if let Some(result) = evaluate_sum_of_constants(&Expression::Sum(expressions.clone())) {
                Expression::Constant(Constant::Int(result))
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
        Expression::Constant(Constant::Int(1)),
        Expression::Constant(Constant::Int(2)),
        Expression::Constant(Constant::Int(3)),
    ]);

    expr = sum_constants.apply(&expr).unwrap();
    expr = unwrap_sum.apply(&expr).unwrap();

    assert_eq!(expr, Expression::Constant(Constant::Int(6)));
}

#[test]
fn rule_sum_mixed() {
    let sum_constants = get_rule_by_name("sum_constants").unwrap();

    let mut expr = Expression::Sum(vec![
        Expression::Constant(Constant::Int(1)),
        Expression::Constant(Constant::Int(2)),
        Expression::Reference(Name::UserName(String::from("a"))),
    ]);

    expr = sum_constants.apply(&expr).unwrap();

    assert_eq!(
        expr,
        Expression::Sum(vec![
            Expression::Reference(Name::UserName(String::from("a"))),
            Expression::Constant(Constant::Int(3)),
        ])
    );
}

#[test]
fn rule_sum_geq() {
    let flatten_sum_geq = get_rule_by_name("flatten_sum_geq").unwrap();

    let mut expr = Expression::Geq(
        Box::new(Expression::Sum(vec![
            Expression::Constant(Constant::Int(1)),
            Expression::Constant(Constant::Int(2)),
        ])),
        Box::new(Expression::Constant(Constant::Int(3))),
    );

    expr = flatten_sum_geq.apply(&expr).unwrap();

    assert_eq!(
        expr,
        Expression::SumGeq(
            vec![
                Expression::Constant(Constant::Int(1)),
                Expression::Constant(Constant::Int(2)),
            ],
            Box::new(Expression::Constant(Constant::Int(3)))
        )
    );
}

fn callback(solution: HashMap<VarName, MinionConstant>) -> bool {
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
        Expression::Constant(Constant::Int(2)),
        Expression::Constant(Constant::Int(3)),
        Expression::Constant(Constant::Int(-1)),
    ]);

    expr1 = sum_constants.apply(&expr1).unwrap();
    expr1 = unwrap_sum.apply(&expr1).unwrap();
    assert_eq!(expr1, Expression::Constant(Constant::Int(4)));

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
            Box::new(Expression::Constant(Constant::Int(4)))
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
            Box::new(Expression::Constant(Constant::Int(-1)))
        )
    );

    let mut model = Model {
        variables: HashMap::new(),
        constraints: Expression::And(vec![expr1, expr2]),
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

#[test]
fn rule_remove_double_negation() {
    let remove_double_negation = get_rule_by_name("remove_double_negation").unwrap();

    let mut expr = Expression::Not(Box::new(Expression::Not(Box::new(Expression::Constant(
        Constant::Bool(true),
    )))));

    expr = remove_double_negation.apply(&expr).unwrap();

    assert_eq!(expr, Expression::Constant(Constant::Bool(true)));
}

#[test]
fn rule_unwrap_nested_or() {
    let unwrap_nested_or = get_rule_by_name("unwrap_nested_or").unwrap();

    let mut expr = Expression::Or(vec![
        Expression::Or(vec![
            Expression::Constant(Constant::Bool(true)),
            Expression::Constant(Constant::Bool(false)),
        ]),
        Expression::Constant(Constant::Bool(true)),
    ]);

    expr = unwrap_nested_or.apply(&expr).unwrap();

    assert_eq!(
        expr,
        Expression::Or(vec![
            Expression::Constant(Constant::Bool(true)),
            Expression::Constant(Constant::Bool(false)),
            Expression::Constant(Constant::Bool(true)),
        ])
    );
}

#[test]
fn rule_unwrap_nested_and() {
    let unwrap_nested_and = get_rule_by_name("unwrap_nested_and").unwrap();

    let mut expr = Expression::And(vec![
        Expression::And(vec![
            Expression::Constant(Constant::Bool(true)),
            Expression::Constant(Constant::Bool(false)),
        ]),
        Expression::Constant(Constant::Bool(true)),
    ]);

    expr = unwrap_nested_and.apply(&expr).unwrap();

    assert_eq!(
        expr,
        Expression::And(vec![
            Expression::Constant(Constant::Bool(true)),
            Expression::Constant(Constant::Bool(false)),
            Expression::Constant(Constant::Bool(true)),
        ])
    );
}

#[test]
fn unwrap_nested_or_not_changed() {
    let unwrap_nested_or = get_rule_by_name("unwrap_nested_or").unwrap();

    let expr = Expression::Or(vec![
        Expression::Constant(Constant::Bool(true)),
        Expression::Constant(Constant::Bool(false)),
    ]);

    let result = unwrap_nested_or.apply(&expr);

    assert!(result.is_err());
}

#[test]
fn unwrap_nested_and_not_changed() {
    let unwrap_nested_and = get_rule_by_name("unwrap_nested_and").unwrap();

    let expr = Expression::And(vec![
        Expression::Constant(Constant::Bool(true)),
        Expression::Constant(Constant::Bool(false)),
    ]);

    let result = unwrap_nested_and.apply(&expr);

    assert!(result.is_err());
}

#[test]
fn remove_trivial_and_or() {
    let remove_trivial_and = get_rule_by_name("remove_trivial_and").unwrap();
    let remove_trivial_or = get_rule_by_name("remove_trivial_or").unwrap();

    let mut expr_and = Expression::And(vec![Expression::Constant(Constant::Bool(true))]);
    let mut expr_or = Expression::Or(vec![Expression::Constant(Constant::Bool(false))]);

    expr_and = remove_trivial_and.apply(&expr_and).unwrap();
    expr_or = remove_trivial_or.apply(&expr_or).unwrap();

    assert_eq!(expr_and, Expression::Constant(Constant::Bool(true)));
    assert_eq!(expr_or, Expression::Constant(Constant::Bool(false)));
}

#[test]
fn rule_remove_constants_from_or() {
    let remove_constants_from_or = get_rule_by_name("remove_constants_from_or").unwrap();

    let mut expr = Expression::Or(vec![
        Expression::Constant(Constant::Bool(true)),
        Expression::Constant(Constant::Bool(false)),
        Expression::Reference(Name::UserName(String::from("a"))),
    ]);

    expr = remove_constants_from_or.apply(&expr).unwrap();

    assert_eq!(expr, Expression::Constant(Constant::Bool(true)));
}

#[test]
fn rule_remove_constants_from_and() {
    let remove_constants_from_and = get_rule_by_name("remove_constants_from_and").unwrap();

    let mut expr = Expression::And(vec![
        Expression::Constant(Constant::Bool(true)),
        Expression::Constant(Constant::Bool(false)),
        Expression::Reference(Name::UserName(String::from("a"))),
    ]);

    expr = remove_constants_from_and.apply(&expr).unwrap();

    assert_eq!(expr, Expression::Constant(Constant::Bool(false)));
}

#[test]
fn remove_constants_from_or_not_changed() {
    let remove_constants_from_or = get_rule_by_name("remove_constants_from_or").unwrap();

    let expr = Expression::Or(vec![
        Expression::Reference(Name::UserName(String::from("a"))),
        Expression::Reference(Name::UserName(String::from("b"))),
    ]);

    let result = remove_constants_from_or.apply(&expr);

    assert!(result.is_err());
}

#[test]
fn remove_constants_from_and_not_changed() {
    let remove_constants_from_and = get_rule_by_name("remove_constants_from_and").unwrap();

    let expr = Expression::And(vec![
        Expression::Reference(Name::UserName(String::from("a"))),
        Expression::Reference(Name::UserName(String::from("b"))),
    ]);

    let result = remove_constants_from_and.apply(&expr);

    assert!(result.is_err());
}

#[test]
fn rule_distribute_not_over_and() {
    let distribute_not_over_and = get_rule_by_name("distribute_not_over_and").unwrap();

    let mut expr = Expression::Not(Box::new(Expression::And(vec![
        Expression::Reference(Name::UserName(String::from("a"))),
        Expression::Reference(Name::UserName(String::from("b"))),
    ])));

    expr = distribute_not_over_and.apply(&expr).unwrap();

    assert_eq!(
        expr,
        Expression::Or(vec![
            Expression::Not(Box::new(Expression::Reference(Name::UserName(
                String::from("a")
            )))),
            Expression::Not(Box::new(Expression::Reference(Name::UserName(
                String::from("b")
            )))),
        ])
    );
}

#[test]
fn rule_distribute_not_over_or() {
    let distribute_not_over_or = get_rule_by_name("distribute_not_over_or").unwrap();

    let mut expr = Expression::Not(Box::new(Expression::Or(vec![
        Expression::Reference(Name::UserName(String::from("a"))),
        Expression::Reference(Name::UserName(String::from("b"))),
    ])));

    expr = distribute_not_over_or.apply(&expr).unwrap();

    assert_eq!(
        expr,
        Expression::And(vec![
            Expression::Not(Box::new(Expression::Reference(Name::UserName(
                String::from("a")
            )))),
            Expression::Not(Box::new(Expression::Reference(Name::UserName(
                String::from("b")
            )))),
        ])
    );
}

#[test]
fn rule_distribute_not_over_and_not_changed() {
    let distribute_not_over_and = get_rule_by_name("distribute_not_over_and").unwrap();

    let expr = Expression::Not(Box::new(Expression::Reference(Name::UserName(
        String::from("a"),
    ))));

    let result = distribute_not_over_and.apply(&expr);

    assert!(result.is_err());
}

#[test]
fn rule_distribute_not_over_or_not_changed() {
    let distribute_not_over_or = get_rule_by_name("distribute_not_over_or").unwrap();

    let expr = Expression::Not(Box::new(Expression::Reference(Name::UserName(
        String::from("a"),
    ))));

    let result = distribute_not_over_or.apply(&expr);

    assert!(result.is_err());
}

#[test]
fn rule_distribute_or_over_and() {
    let distribute_or_over_and = get_rule_by_name("distribute_or_over_and").unwrap();

    let mut expr = Expression::Or(vec![
        Expression::And(vec![
            Expression::Reference(Name::MachineName(1)),
            Expression::Reference(Name::MachineName(2)),
        ]),
        Expression::Reference(Name::MachineName(3)),
    ]);

    expr = distribute_or_over_and.apply(&expr).unwrap();

    assert_eq!(
        expr,
        Expression::And(vec![
            Expression::Or(vec![
                Expression::Reference(Name::MachineName(3)),
                Expression::Reference(Name::MachineName(1)),
            ]),
            Expression::Or(vec![
                Expression::Reference(Name::MachineName(3)),
                Expression::Reference(Name::MachineName(2)),
            ]),
        ]),
    );
}

#[test]
fn eval_const_int() {
    let expr = Expression::Constant(Constant::Int(1));
    let result = eval_constant(&expr);
    assert_eq!(result, Some(Constant::Int(1)));
}

#[test]
fn eval_const_bool() {
    let expr = Expression::Constant(Constant::Bool(true));
    let result = eval_constant(&expr);
    assert_eq!(result, Some(Constant::Bool(true)));
}

#[test]
fn eval_const_and() {
    let expr = Expression::And(vec![
        Expression::Constant(Constant::Bool(true)),
        Expression::Constant(Constant::Bool(false)),
    ]);
    let result = eval_constant(&expr);
    assert_eq!(result, Some(Constant::Bool(false)));
}

#[test]
fn eval_const_ref() {
    let expr = Expression::Reference(Name::UserName(String::from("a")));
    let result = eval_constant(&expr);
    assert_eq!(result, None);
}

#[test]
fn eval_const_nested_ref() {
    let expr = Expression::Sum(vec![
        Expression::Constant(Constant::Int(1)),
        Expression::And(vec![
            Expression::Constant(Constant::Bool(true)),
            Expression::Reference(Name::UserName(String::from("a"))),
        ]),
    ]);
    let result = eval_constant(&expr);
    assert_eq!(result, None);
}

#[test]
fn eval_const_eq_int() {
    let expr = Expression::Eq(
        Box::new(Expression::Constant(Constant::Int(1))),
        Box::new(Expression::Constant(Constant::Int(1))),
    );
    let result = eval_constant(&expr);
    assert_eq!(result, Some(Constant::Bool(true)));
}

#[test]
fn eval_const_eq_bool() {
    let expr = Expression::Eq(
        Box::new(Expression::Constant(Constant::Bool(true))),
        Box::new(Expression::Constant(Constant::Bool(true))),
    );
    let result = eval_constant(&expr);
    assert_eq!(result, Some(Constant::Bool(true)));
}

#[test]
fn eval_const_eq_mixed() {
    let expr = Expression::Eq(
        Box::new(Expression::Constant(Constant::Int(1))),
        Box::new(Expression::Constant(Constant::Bool(true))),
    );
    let result = eval_constant(&expr);
    assert_eq!(result, None);
}

#[test]
fn eval_const_sum_mixed() {
    let expr = Expression::Sum(vec![
        Expression::Constant(Constant::Int(1)),
        Expression::Constant(Constant::Bool(true)),
    ]);
    let result = eval_constant(&expr);
    assert_eq!(result, None);
}

#[test]
fn eval_const_sum_xyz() {
    let expr = Expression::And(vec![
        Expression::Eq(
            Box::new(Expression::Sum(vec![
                Expression::Reference(Name::UserName(String::from("x"))),
                Expression::Reference(Name::UserName(String::from("y"))),
                Expression::Reference(Name::UserName(String::from("z"))),
            ])),
            Box::new(Expression::Constant(Constant::Int(4))),
        ),
        Expression::Geq(
            Box::new(Expression::Reference(Name::UserName(String::from("x")))),
            Box::new(Expression::Reference(Name::UserName(String::from("y")))),
        ),
    ]);
    let result = eval_constant(&expr);
    assert_eq!(result, None);
}

#[test]
fn eval_const_or() {
    let expr = Expression::Or(vec![
        Expression::Constant(Constant::Bool(false)),
        Expression::Constant(Constant::Bool(false)),
    ]);
    let result = eval_constant(&expr);
    assert_eq!(result, Some(Constant::Bool(false)));
}
