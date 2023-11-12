use std::collections::HashMap;

use conjure_oxide::ast::*;

fn main() {
    // find a,b,c : int(1..3)
    // such that a + b + c = 4
    // such that a >= b

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

    println!("{:?}\n", m);
}
