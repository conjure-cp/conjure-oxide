// Tests for various functionalities of the Model

use std::collections::HashMap;
use conjure_oxide::ast::*;

#[test]
fn modify_domain() {
    let a = Name::UserName(String::from("a"));

    let d1 = Domain::IntDomain(vec![Range::Bounded(1, 3)]);
    let d2 = Domain::IntDomain(vec![Range::Bounded(1, 2)]);

    let mut variables = HashMap::new();
    variables.insert(
        a.clone(),
        DecisionVariable {
            domain: d1.clone(),
        },
    );

    let mut m = Model {
        variables,
        constraints: Vec::new(),
    };

    assert!(
        (*m.variables.get(&a).unwrap()).domain == d1
    );

    m.update_domain(&a, d2.clone());

    assert!(
        (*m.variables.get(&a).unwrap()).domain == d2
    );
}
