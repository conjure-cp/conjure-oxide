// Tests for rewriting/simplifying parts of the AST

use conjure_core::rule::Rule;
use conjure_rules::get_rules;
use core::panic;
use std::collections::HashMap;

use conjure_oxide::{ast::*, rewrite::rewrite, solvers::FromConjureModel};
use conjure_rules::get_rule_by_name;
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

    let mut expr = Expression::Not(Box::new(Expression::Not(Box::new(
        Expression::ConstantBool(true),
    ))));

    expr = remove_double_negation.apply(&expr).unwrap();

    assert_eq!(expr, Expression::ConstantBool(true));
}

#[test]
fn rule_unwrap_nested_or() {
    let unwrap_nested_or = get_rule_by_name("unwrap_nested_or").unwrap();

    let mut expr = Expression::Or(vec![
        Expression::Or(vec![
            Expression::ConstantBool(true),
            Expression::ConstantBool(false),
        ]),
        Expression::ConstantBool(true),
    ]);

    expr = unwrap_nested_or.apply(&expr).unwrap();

    assert_eq!(
        expr,
        Expression::Or(vec![
            Expression::ConstantBool(true),
            Expression::ConstantBool(false),
            Expression::ConstantBool(true),
        ])
    );
}

#[test]
fn rule_unwrap_nested_and() {
    let unwrap_nested_and = get_rule_by_name("unwrap_nested_and").unwrap();

    let mut expr = Expression::And(vec![
        Expression::And(vec![
            Expression::ConstantBool(true),
            Expression::ConstantBool(false),
        ]),
        Expression::ConstantBool(true),
    ]);

    expr = unwrap_nested_and.apply(&expr).unwrap();

    assert_eq!(
        expr,
        Expression::And(vec![
            Expression::ConstantBool(true),
            Expression::ConstantBool(false),
            Expression::ConstantBool(true),
        ])
    );
}

#[test]
fn unwrap_nested_or_not_changed() {
    let unwrap_nested_or = get_rule_by_name("unwrap_nested_or").unwrap();

    let expr = Expression::Or(vec![
        Expression::ConstantBool(true),
        Expression::ConstantBool(false),
    ]);

    let result = unwrap_nested_or.apply(&expr);

    assert!(result.is_err());
}

#[test]
fn unwrap_nested_and_not_changed() {
    let unwrap_nested_and = get_rule_by_name("unwrap_nested_and").unwrap();

    let expr = Expression::And(vec![
        Expression::ConstantBool(true),
        Expression::ConstantBool(false),
    ]);

    let result = unwrap_nested_and.apply(&expr);

    assert!(result.is_err());
}

#[test]
fn remove_trivial_and_or() {
    let remove_trivial_and = get_rule_by_name("remove_trivial_and").unwrap();
    let remove_trivial_or = get_rule_by_name("remove_trivial_or").unwrap();

    let mut expr_and = Expression::And(vec![Expression::ConstantBool(true)]);
    let mut expr_or = Expression::Or(vec![Expression::ConstantBool(false)]);

    expr_and = remove_trivial_and.apply(&expr_and).unwrap();
    expr_or = remove_trivial_or.apply(&expr_or).unwrap();

    assert_eq!(expr_and, Expression::ConstantBool(true));
    assert_eq!(expr_or, Expression::ConstantBool(false));
}

#[test]
fn rule_remove_constants_from_or() {
    let remove_constants_from_or = get_rule_by_name("remove_constants_from_or").unwrap();

    let mut expr = Expression::Or(vec![
        Expression::ConstantBool(true),
        Expression::ConstantBool(false),
        Expression::Reference(Name::UserName(String::from("a"))),
    ]);

    expr = remove_constants_from_or.apply(&expr).unwrap();

    assert_eq!(expr, Expression::ConstantBool(true));
}

#[test]
fn rule_remove_constants_from_and() {
    let remove_constants_from_and = get_rule_by_name("remove_constants_from_and").unwrap();

    let mut expr = Expression::And(vec![
        Expression::ConstantBool(true),
        Expression::ConstantBool(false),
        Expression::Reference(Name::UserName(String::from("a"))),
    ]);

    expr = remove_constants_from_and.apply(&expr).unwrap();

    assert_eq!(expr, Expression::ConstantBool(false));
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
    println!("Rules: {:?}", conjure_rules::get_rules());

    // Create variables and domains
    let variable_a = Name::UserName(String::from("a"));
    let variable_b = Name::UserName(String::from("b"));
    let variable_c = Name::UserName(String::from("c"));
    let domain = Domain::IntDomain(vec![Range::Bounded(1, 3)]);

    // Construct nested expression
    let nested_expr = Expression::And(vec![
        Expression::Eq(
            Box::new(Expression::Sum(vec![
                Expression::Reference(variable_a.clone()),
                Expression::Reference(variable_b.clone()),
                Expression::Reference(variable_c.clone()),
            ])),
            Box::new(Expression::ConstantInt(4)),
        ),
        Expression::Lt(
            Box::new(Expression::Reference(variable_a.clone())),
            Box::new(Expression::Reference(variable_b.clone())),
        ),
    ]);

    // Apply rewrite function to the nested expression
    let rewritten_expr = rewrite(&nested_expr);

    // Check if the expression is in its simplest form
    let expr = rewritten_expr.clone();
    assert!(is_simple(&expr));

    // Create model with variables and constraints
    let mut model = Model {
        variables: HashMap::new(),
        constraints: rewritten_expr,
    };

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

    // Convert the model to MinionModel
    let minion_model = conjure_oxide::solvers::minion::MinionModel::from_conjure(model).unwrap();

    // Run the solver with the rewritten model
    minion_rs::run_minion(minion_model, callback).unwrap();
}

struct RuleResult<'a> {
    rule: Rule<'a>,
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
    rules: &'a Vec<Rule<'a>>,
) -> Option<Expression> {
    let rule_results = apply_all_rules(expression, rules);
    if let Some(new) = choose_rewrite(&rule_results) {
        return Some(new);
    } else {
        match expression.sub_expressions() {
            None => {}
            Some(mut sub) => {
                for i in 0..sub.len() {
                    if let Some(new) = is_simple_iteration(sub[i], rules) {
                        sub[i] = &new;
                        return Some(expression.with_sub_expressions(sub));
                    }
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
    rules: &'a Vec<Rule<'a>>,
) -> Vec<RuleResult<'a>> {
    let mut results = Vec::new();
    for rule in rules {
        match rule.apply(expression) {
            Ok(new) => {
                results.push(RuleResult {
                    rule: rule.clone(),
                    new_expression: new,
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
fn choose_rewrite(results: &Vec<RuleResult>) -> Option<Expression> {
    if results.is_empty() {
        return None;
    }
    // Return the first result for now
    // println!("Applying rule: {:?}", results[0].rule);
    Some(results[0].new_expression.clone())
}
