use std::collections::VecDeque;
use std::process::exit;
use std::rc::Rc;

use conjure_core::rule_engine::get_all_rules;
use conjure_core::rule_engine::rewrite_naive;
use conjure_core::{into_matrix_expr, matrix_expr};
use conjure_oxide::SolverFamily;
use conjure_oxide::{
    ast::*,
    get_rule_by_name,
    rule_engine::resolve_rule_sets,
    solver::{adaptors, Solver},
    Metadata, Model, Rule,
};
use conjure_rules::eval_constant;
use uniplate::{Biplate, Uniplate};

fn var_name_from_atom(a: &Atom) -> Name {
    let names: VecDeque<Name> = a.universe_bi();
    names[0].clone()
}
#[test]
fn rules_present() {
    let rules = get_all_rules();
    assert!(!rules.is_empty());
}

#[test]
fn sum_of_constants() {
    let valid_sum_expression = Expression::Sum(
        Metadata::new(),
        Box::new(matrix_expr![
            Expression::Atomic(Metadata::new(), Atom::Literal(Literal::Int(1))),
            Expression::Atomic(Metadata::new(), Atom::Literal(Literal::Int(2))),
            Expression::Atomic(Metadata::new(), Atom::Literal(Literal::Int(3))),
        ]),
    );

    let invalid_sum_expression = Expression::Sum(
        Metadata::new(),
        Box::new(matrix_expr![
            Expression::Atomic(Metadata::new(), Atom::Literal(Literal::Int(1))),
            Expression::Atomic(
                Metadata::new(),
                Atom::Reference(Name::UserName(String::from("a"))),
            ),
        ]),
    );

    assert_eq!(evaluate_sum_of_constants(&valid_sum_expression), Some(6));

    assert_eq!(evaluate_sum_of_constants(&invalid_sum_expression), None);
}

fn evaluate_sum_of_constants(expr: &Expression) -> Option<i32> {
    match expr {
        Expression::Sum(_metadata, expressions) => {
            let expressions = expressions.clone().unwrap_list()?;
            let mut sum = 0;
            for e in expressions {
                match e {
                    Expression::Atomic(_, Atom::Literal(Literal::Int(value))) => {
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
            Box::new(matrix_expr![
                Expression::Atomic(Metadata::new(), Atom::Literal(Literal::Int(1))),
                Expression::Atomic(Metadata::new(), Atom::Literal(Literal::Int(2))),
                Expression::Sum(
                    Metadata::new(),
                    Box::new(matrix_expr![
                        Expression::Atomic(Metadata::new(), Atom::Literal(Literal::Int(1))),
                        Expression::Atomic(Metadata::new(), Atom::Literal(Literal::Int(2))),
                    ]),
                ),
                Expression::Atomic(
                    Metadata::new(),
                    Atom::Reference(Name::UserName(String::from("a"))),
                ),
            ]),
        )),
        Box::new(Expression::Atomic(
            Metadata::new(),
            Atom::Literal(Literal::Int(3)),
        )),
    );
    let correct_simplified_expression = Expression::Eq(
        Metadata::new(),
        Box::new(Expression::Sum(
            Metadata::new(),
            Box::new(matrix_expr![
                Expression::Atomic(Metadata::new(), Atom::Literal(Literal::Int(1))),
                Expression::Atomic(Metadata::new(), Atom::Literal(Literal::Int(2))),
                Expression::Atomic(Metadata::new(), Atom::Literal(Literal::Int(3))),
                Expression::Atomic(
                    Metadata::new(),
                    Atom::Reference(Name::UserName(String::from("a"))),
                ),
            ]),
        )),
        Box::new(Expression::Atomic(
            Metadata::new(),
            Atom::Literal(Literal::Int(3)),
        )),
    );

    let simplified_expression = simplify_expression(complex_expression.clone());
    assert_eq!(simplified_expression, correct_simplified_expression);
}

fn simplify_expression(expr: Expression) -> Expression {
    match expr {
        Expression::Sum(_metadata, expressions) => {
            let expressions = expressions.unwrap_list().unwrap();
            if let Some(result) = evaluate_sum_of_constants(&Expression::Sum(
                Metadata::new(),
                Box::new(into_matrix_expr![expressions.clone()]),
            )) {
                Expression::Atomic(Metadata::new(), Atom::Literal(Literal::Int(result)))
            } else {
                Expression::Sum(
                    Metadata::new(),
                    Box::new(into_matrix_expr![expressions
                        .into_iter()
                        .map(simplify_expression)
                        .collect()]),
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
    let sum_constants = get_rule_by_name("partial_evaluator").unwrap();
    let unwrap_sum = get_rule_by_name("remove_unit_vector_sum").unwrap();

    let mut expr = Expression::Sum(
        Metadata::new(),
        Box::new(matrix_expr![
            Expression::Atomic(Metadata::new(), Atom::Literal(Literal::Int(1))),
            Expression::Atomic(Metadata::new(), Atom::Literal(Literal::Int(2))),
            Expression::Atomic(Metadata::new(), Atom::Literal(Literal::Int(3))),
        ]),
    );

    expr = sum_constants
        .apply(&expr, &SymbolTable::new())
        .unwrap()
        .new_expression;
    expr = unwrap_sum
        .apply(&expr, &SymbolTable::new())
        .unwrap()
        .new_expression;

    assert_eq!(
        expr,
        Expression::Atomic(Metadata::new(), Atom::Literal(Literal::Int(6)))
    );
}

#[test]
fn rule_sum_geq() {
    let introduce_sumgeq = get_rule_by_name("introduce_weighted_sumleq_sumgeq").unwrap();

    let mut expr = Expression::Geq(
        Metadata::new(),
        Box::new(Expression::Sum(
            Metadata::new(),
            Box::new(matrix_expr![
                Expression::Atomic(Metadata::new(), Atom::Literal(Literal::Int(1))),
                Expression::Atomic(Metadata::new(), Atom::Literal(Literal::Int(2))),
            ]),
        )),
        Box::new(Expression::Atomic(
            Metadata::new(),
            Atom::Literal(Literal::Int(3)),
        )),
    );

    expr = introduce_sumgeq
        .apply(&expr, &SymbolTable::new())
        .unwrap()
        .new_expression;

    assert_eq!(
        expr,
        Expression::FlatSumGeq(
            Metadata::new(),
            vec![
                Atom::Literal(Literal::Int(1)),
                Atom::Literal(Literal::Int(2)),
            ],
            Atom::Literal(Literal::Int(3))
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
    println!("Rules: {:?}", get_all_rules());
    let sum_constants = get_rule_by_name("partial_evaluator").unwrap();
    let unwrap_sum = get_rule_by_name("remove_unit_vector_sum").unwrap();
    let lt_to_leq = get_rule_by_name("lt_to_leq").unwrap();
    let leq_to_ineq = get_rule_by_name("x_leq_y_plus_k_to_ineq").unwrap();
    let introduce_sumleq = get_rule_by_name("introduce_weighted_sumleq_sumgeq").unwrap();

    // 2 + 3 - 1
    let mut expr1 = Expression::Sum(
        Metadata::new(),
        Box::new(matrix_expr![
            Expression::Atomic(Metadata::new(), Atom::Literal(Literal::Int(2))),
            Expression::Atomic(Metadata::new(), Atom::Literal(Literal::Int(3))),
            Expression::Atomic(Metadata::new(), Atom::Literal(Literal::Int(-1))),
        ]),
    );

    expr1 = sum_constants
        .apply(&expr1, &SymbolTable::new())
        .unwrap()
        .new_expression;
    expr1 = unwrap_sum
        .apply(&expr1, &SymbolTable::new())
        .unwrap()
        .new_expression;
    assert_eq!(
        expr1,
        Expression::Atomic(Metadata::new(), Atom::Literal(Literal::Int(4)))
    );

    // a + b + c = 4
    expr1 = Expression::Leq(
        Metadata::new(),
        Box::new(Expression::Sum(
            Metadata::new(),
            Box::new(matrix_expr![
                Expression::Atomic(
                    Metadata::new(),
                    Atom::Reference(Name::UserName(String::from("a"))),
                ),
                Expression::Atomic(
                    Metadata::new(),
                    Atom::Reference(Name::UserName(String::from("b"))),
                ),
                Expression::Atomic(
                    Metadata::new(),
                    Atom::Reference(Name::UserName(String::from("c"))),
                ),
            ]),
        )),
        Box::new(expr1),
    );
    expr1 = introduce_sumleq
        .apply(&expr1, &SymbolTable::new())
        .unwrap()
        .new_expression;
    assert_eq!(
        expr1,
        Expression::FlatSumLeq(
            Metadata::new(),
            vec![
                Atom::Reference(Name::UserName(String::from("a"))),
                Atom::Reference(Name::UserName(String::from("b"))),
                Atom::Reference(Name::UserName(String::from("c"))),
            ],
            Atom::Literal(Literal::Int(4))
        )
    );

    // a < b
    let mut expr2 = Expression::Lt(
        Metadata::new(),
        Box::new(Expression::Atomic(
            Metadata::new(),
            Atom::Reference(Name::UserName(String::from("a"))),
        )),
        Box::new(Expression::Atomic(
            Metadata::new(),
            Atom::Reference(Name::UserName(String::from("b"))),
        )),
    );
    expr2 = lt_to_leq
        .apply(&expr2, &SymbolTable::new())
        .unwrap()
        .new_expression;

    expr2 = leq_to_ineq
        .apply(&expr2, &SymbolTable::new())
        .unwrap()
        .new_expression;
    assert_eq!(
        expr2,
        Expression::FlatIneq(
            Metadata::new(),
            Atom::Reference(Name::UserName(String::from("a"))),
            Atom::Reference(Name::UserName(String::from("b"))),
            Literal::Int(-1),
        )
    );

    let mut model = Model::new(Default::default());
    *model.as_submodel_mut().constraints_mut() = vec![expr1, expr2];

    model
        .as_submodel_mut()
        .symbols_mut()
        .insert(Rc::new(Declaration::new_var(
            Name::UserName(String::from("a")),
            Domain::IntDomain(vec![Range::Bounded(1, 3)]),
        )))
        .unwrap();
    model
        .as_submodel_mut()
        .symbols_mut()
        .insert(Rc::new(Declaration::new_var(
            Name::UserName(String::from("b")),
            Domain::IntDomain(vec![Range::Bounded(1, 3)]),
        )))
        .unwrap();
    model
        .as_submodel_mut()
        .symbols_mut()
        .insert(Rc::new(Declaration::new_var(
            Name::UserName(String::from("c")),
            Domain::IntDomain(vec![Range::Bounded(1, 3)]),
        )))
        .unwrap();

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
            Box::new(Expression::Atomic(
                Metadata::new(),
                Atom::Literal(Literal::Bool(true)),
            )),
        )),
    );

    expr = remove_double_negation
        .apply(&expr, &SymbolTable::new())
        .unwrap()
        .new_expression;

    assert_eq!(
        expr,
        Expression::Atomic(Metadata::new(), Atom::Literal(Literal::Bool(true)))
    );
}

#[test]
fn remove_trivial_and_or() {
    let remove_trivial_and = get_rule_by_name("remove_unit_vector_and").unwrap();
    let remove_trivial_or = get_rule_by_name("remove_unit_vector_or").unwrap();

    let mut expr_and = Expression::And(
        Metadata::new(),
        Box::new(matrix_expr![Expression::Atomic(
            Metadata::new(),
            Atom::Literal(Literal::Bool(true)),
        )]),
    );
    let mut expr_or = Expression::Or(
        Metadata::new(),
        Box::new(matrix_expr![Expression::Atomic(
            Metadata::new(),
            Atom::Literal(Literal::Bool(false)),
        )]),
    );

    expr_and = remove_trivial_and
        .apply(&expr_and, &SymbolTable::new())
        .unwrap()
        .new_expression;
    expr_or = remove_trivial_or
        .apply(&expr_or, &SymbolTable::new())
        .unwrap()
        .new_expression;

    assert_eq!(
        expr_and,
        Expression::Atomic(Metadata::new(), Atom::Literal(Literal::Bool(true)))
    );
    assert_eq!(
        expr_or,
        Expression::Atomic(Metadata::new(), Atom::Literal(Literal::Bool(false)))
    );
}

#[test]
fn rule_distribute_not_over_and() {
    let distribute_not_over_and = get_rule_by_name("distribute_not_over_and").unwrap();

    let mut expr = Expression::Not(
        Metadata::new(),
        Box::new(Expression::And(
            Metadata::new(),
            Box::new(matrix_expr![
                Expression::Atomic(
                    Metadata::new(),
                    Atom::Reference(Name::UserName(String::from("a"))),
                ),
                Expression::Atomic(
                    Metadata::new(),
                    Atom::Reference(Name::UserName(String::from("b"))),
                ),
            ]),
        )),
    );

    expr = distribute_not_over_and
        .apply(&expr, &SymbolTable::new())
        .unwrap()
        .new_expression;

    assert_eq!(
        expr,
        Expression::Or(
            Metadata::new(),
            Box::new(matrix_expr![
                Expression::Not(
                    Metadata::new(),
                    Box::new(Expression::Atomic(
                        Metadata::new(),
                        Atom::Reference(Name::UserName(String::from("a")))
                    ))
                ),
                Expression::Not(
                    Metadata::new(),
                    Box::new(Expression::Atomic(
                        Metadata::new(),
                        Atom::Reference(Name::UserName(String::from("b")))
                    ))
                ),
            ])
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
            Box::new(matrix_expr![
                Expression::Atomic(
                    Metadata::new(),
                    Atom::Reference(Name::UserName(String::from("a"))),
                ),
                Expression::Atomic(
                    Metadata::new(),
                    Atom::Reference(Name::UserName(String::from("b"))),
                ),
            ]),
        )),
    );

    expr = distribute_not_over_or
        .apply(&expr, &SymbolTable::new())
        .unwrap()
        .new_expression;

    assert_eq!(
        expr,
        Expression::And(
            Metadata::new(),
            Box::new(matrix_expr![
                Expression::Not(
                    Metadata::new(),
                    Box::new(Expression::Atomic(
                        Metadata::new(),
                        Atom::Reference(Name::UserName(String::from("a")))
                    ))
                ),
                Expression::Not(
                    Metadata::new(),
                    Box::new(Expression::Atomic(
                        Metadata::new(),
                        Atom::Reference(Name::UserName(String::from("b")))
                    ))
                ),
            ])
        )
    );
}

#[test]
fn rule_distribute_not_over_and_not_changed() {
    let distribute_not_over_and = get_rule_by_name("distribute_not_over_and").unwrap();

    let expr = Expression::Not(
        Metadata::new(),
        Box::new(Expression::Atomic(
            Metadata::new(),
            Atom::Reference(Name::UserName(String::from("a"))),
        )),
    );

    let result = distribute_not_over_and.apply(&expr, &SymbolTable::new());

    assert!(result.is_err());
}

#[test]
fn rule_distribute_not_over_or_not_changed() {
    let distribute_not_over_or = get_rule_by_name("distribute_not_over_or").unwrap();

    let expr = Expression::Not(
        Metadata::new(),
        Box::new(Expression::Atomic(
            Metadata::new(),
            Atom::Reference(Name::UserName(String::from("a"))),
        )),
    );

    let result = distribute_not_over_or.apply(&expr, &SymbolTable::new());

    assert!(result.is_err());
}

#[test]
fn rule_distribute_or_over_and() {
    let distribute_or_over_and = get_rule_by_name("distribute_or_over_and").unwrap();

    let expr = Expression::Or(
        Metadata::new(),
        Box::new(matrix_expr![
            Expression::And(
                Metadata::new(),
                Box::new(matrix_expr![
                    Expression::Atomic(Metadata::new(), Atom::Reference(Name::MachineName(1))),
                    Expression::Atomic(Metadata::new(), Atom::Reference(Name::MachineName(2))),
                ]),
            ),
            Expression::Atomic(Metadata::new(), Atom::Reference(Name::MachineName(3))),
        ]),
    );

    let red = distribute_or_over_and
        .apply(&expr, &SymbolTable::new())
        .unwrap();

    assert_eq!(
        red.new_expression,
        Expression::And(
            Metadata::new(),
            Box::new(matrix_expr![
                Expression::Or(
                    Metadata::new(),
                    Box::new(matrix_expr![
                        Expression::Atomic(Metadata::new(), Atom::Reference(Name::MachineName(3))),
                        Expression::Atomic(Metadata::new(), Atom::Reference(Name::MachineName(1))),
                    ])
                ),
                Expression::Or(
                    Metadata::new(),
                    Box::new(matrix_expr![
                        Expression::Atomic(Metadata::new(), Atom::Reference(Name::MachineName(3))),
                        Expression::Atomic(Metadata::new(), Atom::Reference(Name::MachineName(2))),
                    ])
                ),
            ])
        ),
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
    println!("Rules: {:?}", get_all_rules());

    let rule_sets = match resolve_rule_sets(SolverFamily::Minion, &["Constant"]) {
        Ok(rs) => rs,
        Err(e) => {
            eprintln!("Error resolving rule sets: {}", e);
            exit(1);
        }
    };
    println!("Rule sets: {:?}", rule_sets);

    // Create variables and domains
    let variable_a = Atom::Reference(Name::UserName(String::from("a")));
    let variable_b = Atom::Reference(Name::UserName(String::from("b")));
    let variable_c = Atom::Reference(Name::UserName(String::from("c")));
    let domain = Domain::IntDomain(vec![Range::Bounded(1, 3)]);

    // Construct nested expression
    let nested_expr = Expression::And(
        Metadata::new(),
        Box::new(matrix_expr![
            Expression::Eq(
                Metadata::new(),
                Box::new(Expression::Sum(
                    Metadata::new(),
                    Box::new(matrix_expr![
                        Expression::Atomic(Metadata::new(), variable_a.clone()),
                        Expression::Atomic(Metadata::new(), variable_b.clone()),
                        Expression::Atomic(Metadata::new(), variable_c.clone()),
                    ]),
                )),
                Box::new(Expression::Atomic(
                    Metadata::new(),
                    Atom::Literal(Literal::Int(4)),
                )),
            ),
            Expression::Lt(
                Metadata::new(),
                Box::new(Expression::Atomic(Metadata::new(), variable_a.clone())),
                Box::new(Expression::Atomic(Metadata::new(), variable_b.clone())),
            ),
        ]),
    );

    let rule_sets = match resolve_rule_sets(SolverFamily::Minion, &["Constant"]) {
        Ok(rs) => rs,
        Err(e) => {
            eprintln!("Error resolving rule sets: {}", e);
            exit(1);
        }
    };

    // Apply rewrite function to the nested expression
    let mut model = Model::new(Default::default());
    *model.as_submodel_mut().constraints_mut() = vec![nested_expr];
    model = rewrite_naive(&model, &rule_sets, true).unwrap();
    let rewritten_expr = model.as_submodel().constraints();

    // Check if the expression is in its simplest form

    assert!(rewritten_expr.iter().all(is_simple));

    // Insert variables and domains
    model
        .as_submodel_mut()
        .symbols_mut()
        .insert(Rc::new(Declaration::new_var(
            var_name_from_atom(&variable_a.clone()),
            domain.clone(),
        )))
        .unwrap();
    model
        .as_submodel_mut()
        .symbols_mut()
        .insert(Rc::new(Declaration::new_var(
            var_name_from_atom(&variable_b.clone()),
            domain.clone(),
        )))
        .unwrap();
    model
        .as_submodel_mut()
        .symbols_mut()
        .insert(Rc::new(Declaration::new_var(
            var_name_from_atom(&variable_c.clone()),
            domain.clone(),
        )))
        .unwrap();

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
    let rules = get_all_rules();
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
                return Some(expression.with_children(sub.clone()));
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
        match rule.apply(expression, &SymbolTable::new()) {
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
    let expr = Expression::Atomic(Metadata::new(), Atom::Literal(Literal::Int(1)));
    let result = eval_constant(&expr);
    assert_eq!(result, Some(Literal::Int(1)));
}

#[test]
fn eval_const_bool() {
    let expr = Expression::Atomic(Metadata::new(), Atom::Literal(Literal::Bool(true)));
    let result = eval_constant(&expr);
    assert_eq!(result, Some(Literal::Bool(true)));
}

#[test]
fn eval_const_and() {
    let expr = Expression::And(
        Metadata::new(),
        Box::new(matrix_expr![
            Expression::Atomic(Metadata::new(), Atom::Literal(Literal::Bool(true))),
            Expression::Atomic(Metadata::new(), Atom::Literal(Literal::Bool(false))),
        ]),
    );
    let result = eval_constant(&expr);
    assert_eq!(result, Some(Literal::Bool(false)));
}

#[test]
fn eval_const_ref() {
    let expr = Expression::Atomic(
        Metadata::new(),
        Atom::Reference(Name::UserName(String::from("a"))),
    );
    let result = eval_constant(&expr);
    assert_eq!(result, None);
}

#[test]
fn eval_const_nested_ref() {
    let expr = Expression::Sum(
        Metadata::new(),
        Box::new(matrix_expr![
            Expression::Atomic(Metadata::new(), Atom::Literal(Literal::Int(1))),
            Expression::And(
                Metadata::new(),
                Box::new(matrix_expr![
                    Expression::Atomic(Metadata::new(), Atom::Literal(Literal::Bool(true))),
                    Expression::Atomic(
                        Metadata::new(),
                        Atom::Reference(Name::UserName(String::from("a"))),
                    ),
                ]),
            ),
        ]),
    );
    let result = eval_constant(&expr);
    assert_eq!(result, None);
}

#[test]
fn eval_const_eq_int() {
    let expr = Expression::Eq(
        Metadata::new(),
        Box::new(Expression::Atomic(
            Metadata::new(),
            Atom::Literal(Literal::Int(1)),
        )),
        Box::new(Expression::Atomic(
            Metadata::new(),
            Atom::Literal(Literal::Int(1)),
        )),
    );
    let result = eval_constant(&expr);
    assert_eq!(result, Some(Literal::Bool(true)));
}

#[test]
fn eval_const_eq_bool() {
    let expr = Expression::Eq(
        Metadata::new(),
        Box::new(Expression::Atomic(
            Metadata::new(),
            Atom::Literal(Literal::Bool(true)),
        )),
        Box::new(Expression::Atomic(
            Metadata::new(),
            Atom::Literal(Literal::Bool(true)),
        )),
    );
    let result = eval_constant(&expr);
    assert_eq!(result, Some(Literal::Bool(true)));
}

#[test]
fn eval_const_eq_mixed() {
    let expr = Expression::Eq(
        Metadata::new(),
        Box::new(Expression::Atomic(
            Metadata::new(),
            Atom::Literal(Literal::Int(1)),
        )),
        Box::new(Expression::Atomic(
            Metadata::new(),
            Atom::Literal(Literal::Bool(true)),
        )),
    );
    let result = eval_constant(&expr);
    assert_eq!(result, None);
}

#[test]
fn eval_const_sum_mixed() {
    let expr = Expression::Sum(
        Metadata::new(),
        Box::new(matrix_expr![
            Expression::Atomic(Metadata::new(), Atom::Literal(Literal::Int(1))),
            Expression::Atomic(Metadata::new(), Atom::Literal(Literal::Bool(true))),
        ]),
    );
    let result = eval_constant(&expr);
    assert_eq!(result, None);
}

#[test]
fn eval_const_sum_xyz() {
    let expr = Expression::And(
        Metadata::new(),
        Box::new(matrix_expr![
            Expression::Eq(
                Metadata::new(),
                Box::new(Expression::Sum(
                    Metadata::new(),
                    Box::new(matrix_expr![
                        Expression::Atomic(
                            Metadata::new(),
                            Atom::Reference(Name::UserName(String::from("x"))),
                        ),
                        Expression::Atomic(
                            Metadata::new(),
                            Atom::Reference(Name::UserName(String::from("y"))),
                        ),
                        Expression::Atomic(
                            Metadata::new(),
                            Atom::Reference(Name::UserName(String::from("z"))),
                        ),
                    ])
                )),
                Box::new(Expression::Atomic(
                    Metadata::new(),
                    Atom::Literal(Literal::Int(4)),
                )),
            ),
            Expression::Geq(
                Metadata::new(),
                Box::new(Expression::Atomic(
                    Metadata::new(),
                    Atom::Reference(Name::UserName(String::from("x"))),
                )),
                Box::new(Expression::Atomic(
                    Metadata::new(),
                    Atom::Reference(Name::UserName(String::from("y"))),
                )),
            ),
        ]),
    );
    let result = eval_constant(&expr);
    assert_eq!(result, None);
}

#[test]
fn eval_const_or() {
    let expr = Expression::Or(
        Metadata::new(),
        Box::new(matrix_expr![
            Expression::Atomic(Metadata::new(), Atom::Literal(Literal::Bool(false))),
            Expression::Atomic(Metadata::new(), Atom::Literal(Literal::Bool(false))),
        ]),
    );
    let result = eval_constant(&expr);
    assert_eq!(result, Some(Literal::Bool(false)));
}
