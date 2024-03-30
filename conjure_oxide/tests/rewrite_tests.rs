use core::panic;
use std::collections::HashMap;
use std::process::exit;

use conjure_core::context::Context;
use conjure_core::rules::eval_constant;
use conjure_core::solver::SolverFamily;
use conjure_oxide::{
    ast::*,
    get_rule_by_name, get_rules,
    rule_engine::{resolve_rule_sets, rewrite_model},
    solver::{adaptors, Solver, SolverAdaptor as _},
    Metadata, Model, Rule,
};
use uniplate::uniplate::Uniplate;

#[test]
fn rules_present() {
    let rules = get_rules();
    assert!(!rules.is_empty());
}

#[test]
fn sum_of_constants() {
    let valid_sum_expression = Expression::Sum(
        Metadata::new(),
        vec![
            Expression::Constant(Metadata::new(), Constant::Int(1)),
            Expression::Constant(Metadata::new(), Constant::Int(2)),
            Expression::Constant(Metadata::new(), Constant::Int(3)),
        ],
    );

    let invalid_sum_expression = Expression::Sum(
        Metadata::new(),
        vec![
            Expression::Constant(Metadata::new(), Constant::Int(1)),
            Expression::Reference(Metadata::new(), Name::UserName(String::from("a"))),
        ],
    );

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
        Expression::Sum(_metadata, expressions) => {
            let mut sum = 0;
            for e in expressions {
                match e {
                    Expression::Constant(_, Constant::Int(value)) => {
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
        Metadata::new(),
        Box::new(Expression::Sum(
            Metadata::new(),
            vec![
                Expression::Constant(Metadata::new(), Constant::Int(1)),
                Expression::Constant(Metadata::new(), Constant::Int(2)),
                Expression::Sum(
                    Metadata::new(),
                    vec![
                        Expression::Constant(Metadata::new(), Constant::Int(1)),
                        Expression::Constant(Metadata::new(), Constant::Int(2)),
                    ],
                ),
                Expression::Reference(Metadata::new(), Name::UserName(String::from("a"))),
            ],
        )),
        Box::new(Expression::Constant(Metadata::new(), Constant::Int(3))),
    );
    let correct_simplified_expression = Expression::Eq(
        Metadata::new(),
        Box::new(Expression::Sum(
            Metadata::new(),
            vec![
                Expression::Constant(Metadata::new(), Constant::Int(1)),
                Expression::Constant(Metadata::new(), Constant::Int(2)),
                Expression::Constant(Metadata::new(), Constant::Int(3)),
                Expression::Reference(Metadata::new(), Name::UserName(String::from("a"))),
            ],
        )),
        Box::new(Expression::Constant(Metadata::new(), Constant::Int(3))),
    );

    let simplified_expression = simplify_expression(complex_expression.clone());
    assert_eq!(simplified_expression, correct_simplified_expression);
}

fn simplify_expression(expr: Expression) -> Expression {
    match expr {
        Expression::Sum(_metadata, expressions) => {
            if let Some(result) =
                evaluate_sum_of_constants(&Expression::Sum(Metadata::new(), expressions.clone()))
            {
                Expression::Constant(Metadata::new(), Constant::Int(result))
            } else {
                Expression::Sum(
                    Metadata::new(),
                    expressions.into_iter().map(simplify_expression).collect(),
                )
            }
        }
        Expression::Eq(_metadata, left, right) => Expression::Eq(
            Metadata::new(),
            Box::new(simplify_expression(*left)),
            Box::new(simplify_expression(*right)),
        ),
        Expression::Geq(_metadata, left, right) => Expression::Geq(
            Metadata::new(),
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

    let mut expr = Expression::Sum(
        Metadata::new(),
        vec![
            Expression::Constant(Metadata::new(), Constant::Int(1)),
            Expression::Constant(Metadata::new(), Constant::Int(2)),
            Expression::Constant(Metadata::new(), Constant::Int(3)),
        ],
    );

    expr = sum_constants
        .apply(&expr, &Model::new_empty(Default::default()))
        .unwrap()
        .new_expression;
    expr = unwrap_sum
        .apply(&expr, &Model::new_empty(Default::default()))
        .unwrap()
        .new_expression;

    assert_eq!(
        expr,
        Expression::Constant(Metadata::new(), Constant::Int(6))
    );
}

#[test]
fn rule_sum_mixed() {
    let sum_constants = get_rule_by_name("sum_constants").unwrap();

    let mut expr = Expression::Sum(
        Metadata::new(),
        vec![
            Expression::Constant(Metadata::new(), Constant::Int(1)),
            Expression::Constant(Metadata::new(), Constant::Int(2)),
            Expression::Reference(Metadata::new(), Name::UserName(String::from("a"))),
        ],
    );

    expr = sum_constants
        .apply(&expr, &Model::new_empty(Default::default()))
        .unwrap()
        .new_expression;

    assert_eq!(
        expr,
        Expression::Sum(
            Metadata::new(),
            vec![
                Expression::Reference(Metadata::new(), Name::UserName(String::from("a"))),
                Expression::Constant(Metadata::new(), Constant::Int(3)),
            ]
        )
    );
}

#[test]
fn rule_sum_geq() {
    let flatten_sum_geq = get_rule_by_name("flatten_sum_geq").unwrap();

    let mut expr = Expression::Geq(
        Metadata::new(),
        Box::new(Expression::Sum(
            Metadata::new(),
            vec![
                Expression::Constant(Metadata::new(), Constant::Int(1)),
                Expression::Constant(Metadata::new(), Constant::Int(2)),
            ],
        )),
        Box::new(Expression::Constant(Metadata::new(), Constant::Int(3))),
    );

    expr = flatten_sum_geq
        .apply(&expr, &Model::new_empty(Default::default()))
        .unwrap()
        .new_expression;

    assert_eq!(
        expr,
        Expression::SumGeq(
            Metadata::new(),
            vec![
                Expression::Constant(Metadata::new(), Constant::Int(1)),
                Expression::Constant(Metadata::new(), Constant::Int(2)),
            ],
            Box::new(Expression::Constant(Metadata::new(), Constant::Int(3)))
        )
    );
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
    println!("Rules: {:?}", get_rules());
    let sum_constants = get_rule_by_name("sum_constants").unwrap();
    let unwrap_sum = get_rule_by_name("unwrap_sum").unwrap();
    let lt_to_ineq = get_rule_by_name("lt_to_ineq").unwrap();
    let sum_leq_to_sumleq = get_rule_by_name("sum_leq_to_sumleq").unwrap();

    // 2 + 3 - 1
    let mut expr1 = Expression::Sum(
        Metadata::new(),
        vec![
            Expression::Constant(Metadata::new(), Constant::Int(2)),
            Expression::Constant(Metadata::new(), Constant::Int(3)),
            Expression::Constant(Metadata::new(), Constant::Int(-1)),
        ],
    );

    expr1 = sum_constants
        .apply(&expr1, &Model::new_empty(Default::default()))
        .unwrap()
        .new_expression;
    expr1 = unwrap_sum
        .apply(&expr1, &Model::new_empty(Default::default()))
        .unwrap()
        .new_expression;
    assert_eq!(
        expr1,
        Expression::Constant(Metadata::new(), Constant::Int(4))
    );

    // a + b + c = 4
    expr1 = Expression::Leq(
        Metadata::new(),
        Box::new(Expression::Sum(
            Metadata::new(),
            vec![
                Expression::Reference(Metadata::new(), Name::UserName(String::from("a"))),
                Expression::Reference(Metadata::new(), Name::UserName(String::from("b"))),
                Expression::Reference(Metadata::new(), Name::UserName(String::from("c"))),
            ],
        )),
        Box::new(expr1),
    );
    expr1 = sum_leq_to_sumleq
        .apply(&expr1, &Model::new_empty(Default::default()))
        .unwrap()
        .new_expression;
    assert_eq!(
        expr1,
        Expression::SumLeq(
            Metadata::new(),
            vec![
                Expression::Reference(Metadata::new(), Name::UserName(String::from("a"))),
                Expression::Reference(Metadata::new(), Name::UserName(String::from("b"))),
                Expression::Reference(Metadata::new(), Name::UserName(String::from("c"))),
            ],
            Box::new(Expression::Constant(Metadata::new(), Constant::Int(4)))
        )
    );

    // a < b
    let mut expr2 = Expression::Lt(
        Metadata::new(),
        Box::new(Expression::Reference(
            Metadata::new(),
            Name::UserName(String::from("a")),
        )),
        Box::new(Expression::Reference(
            Metadata::new(),
            Name::UserName(String::from("b")),
        )),
    );
    expr2 = lt_to_ineq
        .apply(&expr2, &Model::new_empty(Default::default()))
        .unwrap()
        .new_expression;
    assert_eq!(
        expr2,
        Expression::Ineq(
            Metadata::new(),
            Box::new(Expression::Reference(
                Metadata::new(),
                Name::UserName(String::from("a"))
            )),
            Box::new(Expression::Reference(
                Metadata::new(),
                Name::UserName(String::from("b"))
            )),
            Box::new(Expression::Constant(Metadata::new(), Constant::Int(-1)))
        )
    );

    let mut model = Model::new(
        HashMap::new(),
        Expression::And(Metadata::new(), vec![expr1, expr2]),
        Default::default(),
    );
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

    let solver: Solver<adaptors::Minion> = Solver::new(adaptors::Minion::new());
    let solver = solver.load_model(model).unwrap();
    solver.solve(Box::new(|_| true)).unwrap();
}

#[test]
fn rule_remove_double_negation() {
    let remove_double_negation = get_rule_by_name("remove_double_negation").unwrap();

    let mut expr = Expression::Not(
        Metadata::new(),
        Box::new(Expression::Not(
            Metadata::new(),
            Box::new(Expression::Constant(Metadata::new(), Constant::Bool(true))),
        )),
    );

    expr = remove_double_negation
        .apply(&expr, &Model::new_empty(Default::default()))
        .unwrap()
        .new_expression;

    assert_eq!(
        expr,
        Expression::Constant(Metadata::new(), Constant::Bool(true))
    );
}

#[test]
fn rule_unwrap_nested_or() {
    let unwrap_nested_or = get_rule_by_name("unwrap_nested_or").unwrap();

    let mut expr = Expression::Or(
        Metadata::new(),
        vec![
            Expression::Or(
                Metadata::new(),
                vec![
                    Expression::Constant(Metadata::new(), Constant::Bool(true)),
                    Expression::Constant(Metadata::new(), Constant::Bool(false)),
                ],
            ),
            Expression::Constant(Metadata::new(), Constant::Bool(true)),
        ],
    );

    expr = unwrap_nested_or
        .apply(&expr, &Model::new_empty(Default::default()))
        .unwrap()
        .new_expression;

    assert_eq!(
        expr,
        Expression::Or(
            Metadata::new(),
            vec![
                Expression::Constant(Metadata::new(), Constant::Bool(true)),
                Expression::Constant(Metadata::new(), Constant::Bool(false)),
                Expression::Constant(Metadata::new(), Constant::Bool(true)),
            ]
        )
    );
}

#[test]
fn rule_unwrap_nested_and() {
    let unwrap_nested_and = get_rule_by_name("unwrap_nested_and").unwrap();

    let mut expr = Expression::And(
        Metadata::new(),
        vec![
            Expression::And(
                Metadata::new(),
                vec![
                    Expression::Constant(Metadata::new(), Constant::Bool(true)),
                    Expression::Constant(Metadata::new(), Constant::Bool(false)),
                ],
            ),
            Expression::Constant(Metadata::new(), Constant::Bool(true)),
        ],
    );

    expr = unwrap_nested_and
        .apply(&expr, &Model::new_empty(Default::default()))
        .unwrap()
        .new_expression;

    assert_eq!(
        expr,
        Expression::And(
            Metadata::new(),
            vec![
                Expression::Constant(Metadata::new(), Constant::Bool(true)),
                Expression::Constant(Metadata::new(), Constant::Bool(false)),
                Expression::Constant(Metadata::new(), Constant::Bool(true)),
            ]
        )
    );
}

#[test]
fn unwrap_nested_or_not_changed() {
    let unwrap_nested_or = get_rule_by_name("unwrap_nested_or").unwrap();

    let expr = Expression::Or(
        Metadata::new(),
        vec![
            Expression::Constant(Metadata::new(), Constant::Bool(true)),
            Expression::Constant(Metadata::new(), Constant::Bool(false)),
        ],
    );

    let result = unwrap_nested_or.apply(&expr, &Model::new_empty(Default::default()));

    assert!(result.is_err());
}

#[test]
fn unwrap_nested_and_not_changed() {
    let unwrap_nested_and = get_rule_by_name("unwrap_nested_and").unwrap();

    let expr = Expression::And(
        Metadata::new(),
        vec![
            Expression::Constant(Metadata::new(), Constant::Bool(true)),
            Expression::Constant(Metadata::new(), Constant::Bool(false)),
        ],
    );

    let result = unwrap_nested_and.apply(&expr, &Model::new_empty(Default::default()));

    assert!(result.is_err());
}

#[test]
fn remove_trivial_and_or() {
    let remove_trivial_and = get_rule_by_name("remove_trivial_and").unwrap();
    let remove_trivial_or = get_rule_by_name("remove_trivial_or").unwrap();

    let mut expr_and = Expression::And(
        Metadata::new(),
        vec![Expression::Constant(Metadata::new(), Constant::Bool(true))],
    );
    let mut expr_or = Expression::Or(
        Metadata::new(),
        vec![Expression::Constant(Metadata::new(), Constant::Bool(false))],
    );

    expr_and = remove_trivial_and
        .apply(&expr_and, &Model::new_empty(Default::default()))
        .unwrap()
        .new_expression;
    expr_or = remove_trivial_or
        .apply(&expr_or, &Model::new_empty(Default::default()))
        .unwrap()
        .new_expression;

    assert_eq!(
        expr_and,
        Expression::Constant(Metadata::new(), Constant::Bool(true))
    );
    assert_eq!(
        expr_or,
        Expression::Constant(Metadata::new(), Constant::Bool(false))
    );
}

#[test]
fn rule_remove_constants_from_or() {
    let remove_constants_from_or = get_rule_by_name("remove_constants_from_or").unwrap();

    let mut expr = Expression::Or(
        Metadata::new(),
        vec![
            Expression::Constant(Metadata::new(), Constant::Bool(true)),
            Expression::Constant(Metadata::new(), Constant::Bool(false)),
            Expression::Reference(Metadata::new(), Name::UserName(String::from("a"))),
        ],
    );

    expr = remove_constants_from_or
        .apply(&expr, &Model::new_empty(Default::default()))
        .unwrap()
        .new_expression;

    assert_eq!(
        expr,
        Expression::Constant(Metadata::new(), Constant::Bool(true))
    );
}

#[test]
fn rule_remove_constants_from_and() {
    let remove_constants_from_and = get_rule_by_name("remove_constants_from_and").unwrap();

    let mut expr = Expression::And(
        Metadata::new(),
        vec![
            Expression::Constant(Metadata::new(), Constant::Bool(true)),
            Expression::Constant(Metadata::new(), Constant::Bool(false)),
            Expression::Reference(Metadata::new(), Name::UserName(String::from("a"))),
        ],
    );

    expr = remove_constants_from_and
        .apply(&expr, &Model::new_empty(Default::default()))
        .unwrap()
        .new_expression;

    assert_eq!(
        expr,
        Expression::Constant(Metadata::new(), Constant::Bool(false))
    );
}

#[test]
fn remove_constants_from_or_not_changed() {
    let remove_constants_from_or = get_rule_by_name("remove_constants_from_or").unwrap();

    let expr = Expression::Or(
        Metadata::new(),
        vec![
            Expression::Reference(Metadata::new(), Name::UserName(String::from("a"))),
            Expression::Reference(Metadata::new(), Name::UserName(String::from("b"))),
        ],
    );

    let result = remove_constants_from_or.apply(&expr, &Model::new_empty(Default::default()));

    assert!(result.is_err());
}

#[test]
fn remove_constants_from_and_not_changed() {
    let remove_constants_from_and = get_rule_by_name("remove_constants_from_and").unwrap();

    let expr = Expression::And(
        Metadata::new(),
        vec![
            Expression::Reference(Metadata::new(), Name::UserName(String::from("a"))),
            Expression::Reference(Metadata::new(), Name::UserName(String::from("b"))),
        ],
    );

    let result = remove_constants_from_and.apply(&expr, &Model::new_empty(Default::default()));

    assert!(result.is_err());
}

#[test]
fn rule_distribute_not_over_and() {
    let distribute_not_over_and = get_rule_by_name("distribute_not_over_and").unwrap();

    let mut expr = Expression::Not(
        Metadata::new(),
        Box::new(Expression::And(
            Metadata::new(),
            vec![
                Expression::Reference(Metadata::new(), Name::UserName(String::from("a"))),
                Expression::Reference(Metadata::new(), Name::UserName(String::from("b"))),
            ],
        )),
    );

    expr = distribute_not_over_and
        .apply(&expr, &Model::new_empty(Default::default()))
        .unwrap()
        .new_expression;

    assert_eq!(
        expr,
        Expression::Or(
            Metadata::new(),
            vec![
                Expression::Not(
                    Metadata::new(),
                    Box::new(Expression::Reference(
                        Metadata::new(),
                        Name::UserName(String::from("a"))
                    ))
                ),
                Expression::Not(
                    Metadata::new(),
                    Box::new(Expression::Reference(
                        Metadata::new(),
                        Name::UserName(String::from("b"))
                    ))
                ),
            ]
        )
    );
}

#[test]
fn rule_distribute_not_over_or() {
    let distribute_not_over_or = get_rule_by_name("distribute_not_over_or").unwrap();

    let mut expr = Expression::Not(
        Metadata::new(),
        Box::new(Expression::Or(
            Metadata::new(),
            vec![
                Expression::Reference(Metadata::new(), Name::UserName(String::from("a"))),
                Expression::Reference(Metadata::new(), Name::UserName(String::from("b"))),
            ],
        )),
    );

    expr = distribute_not_over_or
        .apply(&expr, &Model::new_empty(Default::default()))
        .unwrap()
        .new_expression;

    assert_eq!(
        expr,
        Expression::And(
            Metadata::new(),
            vec![
                Expression::Not(
                    Metadata::new(),
                    Box::new(Expression::Reference(
                        Metadata::new(),
                        Name::UserName(String::from("a"))
                    ))
                ),
                Expression::Not(
                    Metadata::new(),
                    Box::new(Expression::Reference(
                        Metadata::new(),
                        Name::UserName(String::from("b"))
                    ))
                ),
            ]
        )
    );
}

#[test]
fn rule_distribute_not_over_and_not_changed() {
    let distribute_not_over_and = get_rule_by_name("distribute_not_over_and").unwrap();

    let expr = Expression::Not(
        Metadata::new(),
        Box::new(Expression::Reference(
            Metadata::new(),
            Name::UserName(String::from("a")),
        )),
    );

    let result = distribute_not_over_and.apply(&expr, &Model::new_empty(Default::default()));

    assert!(result.is_err());
}

#[test]
fn rule_distribute_not_over_or_not_changed() {
    let distribute_not_over_or = get_rule_by_name("distribute_not_over_or").unwrap();

    let expr = Expression::Not(
        Metadata::new(),
        Box::new(Expression::Reference(
            Metadata::new(),
            Name::UserName(String::from("a")),
        )),
    );

    let result = distribute_not_over_or.apply(&expr, &Model::new_empty(Default::default()));

    assert!(result.is_err());
}

#[test]
fn rule_distribute_or_over_and() {
    let distribute_or_over_and = get_rule_by_name("distribute_or_over_and").unwrap();

    let expr = Expression::Or(
        Metadata::new(),
        vec![
            Expression::And(
                Metadata::new(),
                vec![
                    Expression::Reference(Metadata::new(), Name::MachineName(1)),
                    Expression::Reference(Metadata::new(), Name::MachineName(2)),
                ],
            ),
            Expression::Reference(Metadata::new(), Name::MachineName(3)),
        ],
    );

    let red = distribute_or_over_and
        .apply(&expr, &Model::new_empty(Default::default()))
        .unwrap();

    assert_eq!(
        red.new_expression,
        Expression::And(
            Metadata::new(),
            vec![
                Expression::Or(
                    Metadata::new(),
                    vec![
                        Expression::Reference(Metadata::new(), Name::MachineName(3)),
                        Expression::Reference(Metadata::new(), Name::MachineName(1)),
                    ]
                ),
                Expression::Or(
                    Metadata::new(),
                    vec![
                        Expression::Reference(Metadata::new(), Name::MachineName(3)),
                        Expression::Reference(Metadata::new(), Name::MachineName(2)),
                    ]
                ),
            ]
        ),
    );
}

// #[test]
// fn rule_ensure_div() {
//     let ensure_div = get_rule_by_name("ensure_div").unwrap();

//     let expr = Expression::Div(
//         Metadata::new(),
//         Box::new(Expression::Reference(
//             Metadata::new(),
//             Name::UserName("a".to_string()),
//         )),
//         Box::new(Expression::Reference(
//             Metadata::new(),
//             Name::UserName("b".to_string()),
//         )),
//     );

//     let red = ensure_div.apply(&expr, &Model::new_empty(Default::default())).unwrap();

//     assert_eq!(
//         red.new_expression,
//         Expression::SafeDiv(
//             Metadata::new(),
//             Box::new(Expression::Reference(
//                 Metadata::new(),
//                 Name::UserName("a".to_string())
//             )),
//             Box::new(Expression::Reference(
//                 Metadata::new(),
//                 Name::UserName("b".to_string())
//             )),
//         ),
//     );
//     assert_eq!(
//         red.new_top,
//         Expression::Neq(
//             Metadata::new(),
//             Box::new(Expression::Reference(
//                 Metadata::new(),
//                 Name::UserName("b".to_string())
//             )),
//             Box::new(Expression::Constant(Metadata::new(), Constant::Int(0)))
//         )
//     );
// }

///
/// Reduce and solve:
/// ```text
/// find a,b,c : int(1..3)
/// such that a + b + c = 4
/// such that a < b
/// ```
///
/// This test uses the rewrite function to simplify the expression instead
/// of applying the rules manually.
#[test]
fn rewrite_solve_xyz() {
    println!("Rules: {:?}", get_rules());

    let rule_sets = match resolve_rule_sets(SolverFamily::Minion, &vec!["Constant"]) {
        Ok(rs) => rs,
        Err(e) => {
            eprintln!("Error resolving rule sets: {}", e);
            exit(1);
        }
    };
    println!("Rule sets: {:?}", rule_sets);

    // Create variables and domains
    let variable_a = Name::UserName(String::from("a"));
    let variable_b = Name::UserName(String::from("b"));
    let variable_c = Name::UserName(String::from("c"));
    let domain = Domain::IntDomain(vec![Range::Bounded(1, 3)]);

    // Construct nested expression
    let nested_expr = Expression::And(
        Metadata::new(),
        vec![
            Expression::Eq(
                Metadata::new(),
                Box::new(Expression::Sum(
                    Metadata::new(),
                    vec![
                        Expression::Reference(Metadata::new(), variable_a.clone()),
                        Expression::Reference(Metadata::new(), variable_b.clone()),
                        Expression::Reference(Metadata::new(), variable_c.clone()),
                    ],
                )),
                Box::new(Expression::Constant(Metadata::new(), Constant::Int(4))),
            ),
            Expression::Lt(
                Metadata::new(),
                Box::new(Expression::Reference(Metadata::new(), variable_a.clone())),
                Box::new(Expression::Reference(Metadata::new(), variable_b.clone())),
            ),
        ],
    );

    let rule_sets = match resolve_rule_sets(SolverFamily::Minion, &vec!["Constant"]) {
        Ok(rs) => rs,
        Err(e) => {
            eprintln!("Error resolving rule sets: {}", e);
            exit(1);
        }
    };

    // Apply rewrite function to the nested expression
    let rewritten_expr = rewrite_model(
        &Model::new(HashMap::new(), nested_expr, Default::default()),
        &rule_sets,
    )
    .unwrap()
    .constraints;

    // Check if the expression is in its simplest form
    let expr = rewritten_expr.clone();
    assert!(is_simple(&expr));

    // Create model with variables and constraints
    let mut model = Model::new(HashMap::new(), rewritten_expr, Default::default());

    // Insert variables and domains
    model.variables.insert(
        variable_a.clone(),
        DecisionVariable {
            domain: domain.clone(),
        },
    );
    model.variables.insert(
        variable_b.clone(),
        DecisionVariable {
            domain: domain.clone(),
        },
    );
    model.variables.insert(
        variable_c.clone(),
        DecisionVariable {
            domain: domain.clone(),
        },
    );

    let solver: Solver<adaptors::Minion> = Solver::new(adaptors::Minion::new());
    let solver = solver.load_model(model).unwrap();
    solver.solve(Box::new(|_| true)).unwrap();
}

struct RuleResult<'a> {
    #[allow(dead_code)]
    rule: &'a Rule<'a>,
    new_expression: Expression,
}

/// # Returns
/// - True if `expression` is in its simplest form.
/// - False otherwise.
pub fn is_simple(expression: &Expression) -> bool {
    let rules = get_rules();
    let mut new = expression.clone();
    while let Some(step) = is_simple_iteration(&new, &rules) {
        new = step;
    }
    new == *expression
}

/// # Returns
/// - Some(<new_expression>) after applying the first applicable rule to `expr` or a sub-expression.
/// - None if no rule is applicable to the expression or any sub-expression.
fn is_simple_iteration<'a>(
    expression: &'a Expression,
    rules: &'a Vec<&'a Rule<'a>>,
) -> Option<Expression> {
    let rule_results = apply_all_rules(expression, rules);
    if let Some(new) = choose_rewrite(&rule_results) {
        return Some(new);
    } else {
        let mut sub = expression.children();
        for i in 0..sub.len() {
            if let Some(new) = is_simple_iteration(&sub[i], rules) {
                sub[i] = new;
                if let Ok(res) = expression.with_children(sub.clone()) {
                    return Some(res);
                }
            }
        }
    }
    None // No rules applicable to this branch of the expression
}

/// # Returns
/// - A list of RuleResults after applying all rules to `expression`.
/// - An empty list if no rules are applicable.
fn apply_all_rules<'a>(
    expression: &'a Expression,
    rules: &'a Vec<&'a Rule<'a>>,
) -> Vec<RuleResult<'a>> {
    let mut results = Vec::new();
    for rule in rules {
        match rule.apply(expression, &Model::new_empty(Default::default())) {
            Ok(red) => {
                results.push(RuleResult {
                    rule,
                    new_expression: red.new_expression,
                });
            }
            Err(_) => continue,
        }
    }
    results
}

/// # Returns
/// - Some(<new_expression>) after applying the first rule in `results`.
/// - None if `results` is empty.
fn choose_rewrite(results: &[RuleResult]) -> Option<Expression> {
    if results.is_empty() {
        return None;
    }
    // Return the first result for now
    // println!("Applying rule: {:?}", results[0].rule);
    Some(results[0].new_expression.clone())
}

#[test]
fn eval_const_int() {
    let expr = Expression::Constant(Metadata::new(), Constant::Int(1));
    let result = eval_constant(&expr);
    assert_eq!(result, Some(Constant::Int(1)));
}

#[test]
fn eval_const_bool() {
    let expr = Expression::Constant(Metadata::new(), Constant::Bool(true));
    let result = eval_constant(&expr);
    assert_eq!(result, Some(Constant::Bool(true)));
}

#[test]
fn eval_const_and() {
    let expr = Expression::And(
        Metadata::new(),
        vec![
            Expression::Constant(Metadata::new(), Constant::Bool(true)),
            Expression::Constant(Metadata::new(), Constant::Bool(false)),
        ],
    );
    let result = eval_constant(&expr);
    assert_eq!(result, Some(Constant::Bool(false)));
}

#[test]
fn eval_const_ref() {
    let expr = Expression::Reference(Metadata::new(), Name::UserName(String::from("a")));
    let result = eval_constant(&expr);
    assert_eq!(result, None);
}

#[test]
fn eval_const_nested_ref() {
    let expr = Expression::Sum(
        Metadata::new(),
        vec![
            Expression::Constant(Metadata::new(), Constant::Int(1)),
            Expression::And(
                Metadata::new(),
                vec![
                    Expression::Constant(Metadata::new(), Constant::Bool(true)),
                    Expression::Reference(Metadata::new(), Name::UserName(String::from("a"))),
                ],
            ),
        ],
    );
    let result = eval_constant(&expr);
    assert_eq!(result, None);
}

#[test]
fn eval_const_eq_int() {
    let expr = Expression::Eq(
        Metadata::new(),
        Box::new(Expression::Constant(Metadata::new(), Constant::Int(1))),
        Box::new(Expression::Constant(Metadata::new(), Constant::Int(1))),
    );
    let result = eval_constant(&expr);
    assert_eq!(result, Some(Constant::Bool(true)));
}

#[test]
fn eval_const_eq_bool() {
    let expr = Expression::Eq(
        Metadata::new(),
        Box::new(Expression::Constant(Metadata::new(), Constant::Bool(true))),
        Box::new(Expression::Constant(Metadata::new(), Constant::Bool(true))),
    );
    let result = eval_constant(&expr);
    assert_eq!(result, Some(Constant::Bool(true)));
}

#[test]
fn eval_const_eq_mixed() {
    let expr = Expression::Eq(
        Metadata::new(),
        Box::new(Expression::Constant(Metadata::new(), Constant::Int(1))),
        Box::new(Expression::Constant(Metadata::new(), Constant::Bool(true))),
    );
    let result = eval_constant(&expr);
    assert_eq!(result, None);
}

#[test]
fn eval_const_sum_mixed() {
    let expr = Expression::Sum(
        Metadata::new(),
        vec![
            Expression::Constant(Metadata::new(), Constant::Int(1)),
            Expression::Constant(Metadata::new(), Constant::Bool(true)),
        ],
    );
    let result = eval_constant(&expr);
    assert_eq!(result, None);
}

#[test]
fn eval_const_sum_xyz() {
    let expr = Expression::And(
        Metadata::new(),
        vec![
            Expression::Eq(
                Metadata::new(),
                Box::new(Expression::Sum(
                    Metadata::new(),
                    vec![
                        Expression::Reference(Metadata::new(), Name::UserName(String::from("x"))),
                        Expression::Reference(Metadata::new(), Name::UserName(String::from("y"))),
                        Expression::Reference(Metadata::new(), Name::UserName(String::from("z"))),
                    ],
                )),
                Box::new(Expression::Constant(Metadata::new(), Constant::Int(4))),
            ),
            Expression::Geq(
                Metadata::new(),
                Box::new(Expression::Reference(
                    Metadata::new(),
                    Name::UserName(String::from("x")),
                )),
                Box::new(Expression::Reference(
                    Metadata::new(),
                    Name::UserName(String::from("y")),
                )),
            ),
        ],
    );
    let result = eval_constant(&expr);
    assert_eq!(result, None);
}

#[test]
fn eval_const_or() {
    let expr = Expression::Or(
        Metadata::new(),
        vec![
            Expression::Constant(Metadata::new(), Constant::Bool(false)),
            Expression::Constant(Metadata::new(), Constant::Bool(false)),
        ],
    );
    let result = eval_constant(&expr);
    assert_eq!(result, Some(Constant::Bool(false)));
}
