use std::collections::HashMap;

use conjure_oxide::ast::*;

#[test]
fn abc_equality() {
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

    let m1 = Model {
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

    let m2 = ModelBuilder::new()
        .add_var_str("a", Domain::IntDomain(vec![Range::Bounded(1, 3)]))
        .add_var_str("b", Domain::IntDomain(vec![Range::Bounded(1, 3)]))
        .add_var_str("c", Domain::IntDomain(vec![Range::Bounded(1, 3)]))
        .add_constraint(Expression::Eq(
            Box::new(Expression::Sum(vec![
                Expression::Reference(a.clone()),
                Expression::Reference(b.clone()),
                Expression::Reference(c.clone()),
            ])),
            Box::new(Expression::ConstantInt(4)),
        ))
        .add_constraint(Expression::Geq(
            Box::new(Expression::Reference(a.clone())),
            Box::new(Expression::Reference(b.clone())),
        ))
        .build();

    assert!(m1 == m2);
}
