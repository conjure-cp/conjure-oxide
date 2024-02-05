// Tests for various functionalities of the Model

use conjure_oxide::ast::*;
use std::collections::HashMap;

#[test]
fn modify_domain() {
    let a = Name::UserName(String::from("a"));

    let d1 = Domain::IntDomain(vec![Range::Bounded(1, 3)]);
    let d2 = Domain::IntDomain(vec![Range::Bounded(1, 2)]);

    let mut variables = HashMap::new();
    variables.insert(a.clone(), DecisionVariable { domain: d1.clone() });

    let mut m = Model {
        variables,
        constraints: Expression::And(Vec::new()),
    };

    assert_eq!(m.variables.get(&a).unwrap().domain, d1);

    m.update_domain(&a, d2.clone());

    assert_eq!(m.variables.get(&a).unwrap().domain, d2);
}
