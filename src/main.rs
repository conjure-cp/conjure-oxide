use std::collections::HashMap;

mod common;
use common::ast::*;

// see all mainX functions, each one shows a different thing in action.

// --------------------------------------------------------------------------------
// constructing a model
// and modifying the domain of a decision variable
fn main1() {
    let a = Name::UserName(String::from("a"));
    let b = Name::UserName(String::from("b"));
    let c = Name::UserName(String::from("c"));

    let mut variables = HashMap::new();
    variables.insert(
        a.clone(),
        DecisionVariable {
            domain: Domain::IntDomain(vec![Range::Bounded(1, 3)]),
        },
    );
    variables.insert(
        b.clone(),
        DecisionVariable {
            domain: Domain::IntDomain(vec![Range::Bounded(1, 3)]),
        },
    );
    variables.insert(
        c.clone(),
        DecisionVariable {
            domain: Domain::IntDomain(vec![Range::Bounded(1, 3)]),
        },
    );

    // find a,b,c : int(1..3)
    // such that a + b + c = 4
    // such that a >= b
    let mut m = Model {
        variables,
        constraints: vec![
            Expression::Eq(
                Box::new(Expression::Sum(vec![
                    Expression::Reference(a.clone()),
                    Expression::Reference(b.clone()),
                    Expression::Reference(c.clone()),
                ])),
                Box::new(Expression::ConstantInt(4)),
            ),
            Expression::Geq(
                Box::new(Expression::Reference(a.clone())),
                Box::new(Expression::Reference(b.clone())),
            ),
        ],
    };

    println!("{:#?}", m);

    // Updating the domain for variable 'a'
    m.update_domain(&a, Domain::IntDomain(vec![Range::Bounded(1, 2)]));

    println!("{:#?}", m);
}

// --------------------------------------------------------------------------------
// evaluating a sum of constants down to a single constant, not touching anything else.
fn main2() {
    let valid_sum_expression = Expression::Sum(vec![
        Expression::ConstantInt(1),
        Expression::ConstantInt(2),
        Expression::ConstantInt(3),
    ]);

    let invalid_sum_expression = Expression::Sum(vec![
        Expression::ConstantInt(1),
        Expression::Reference(Name::UserName(String::from("a"))),
    ]);

    if let Some(result) = evaluate_sum_of_constants(&valid_sum_expression) {
        println!("The sum is: {}", result); // Output: The sum is: 6
    } else {
        println!("The expression is not a sum of constant integers.");
    }

    if let Some(result) = evaluate_sum_of_constants(&invalid_sum_expression) {
        println!("The sum is: {}", result);
    } else {
        println!("The expression is not a sum of constant integers."); // This will be printed
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

// --------------------------------------------------------------------------------
// applying evaluate_sum_of_constants recursively
fn main3() {
    let complex_expression = Expression::Eq(
        Box::new(Expression::Sum(vec![
            Expression::ConstantInt(1),
            Expression::ConstantInt(2),
            Expression::Sum(vec![Expression::ConstantInt(1), Expression::ConstantInt(2)]),
            Expression::Reference(Name::UserName(String::from("a"))),
        ])),
        Box::new(Expression::ConstantInt(3)),
    );

    let simplified_expression = simplify_expression(complex_expression.clone());
    println!("Original expression: {:#?}", complex_expression);
    println!("Simplified expression: {:#?}", simplified_expression);
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

// --------------------------------------------------------------------------------

fn main() {
    // main1();
    // main2();
    main3();
}
