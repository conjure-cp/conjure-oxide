// Tests for various functionalities of the Model

use std::collections::BTreeMap;

use conjure_core::model::Model;
use conjure_oxide::ast::*;

#[test]
fn modify_domain() {
    let a = Name::UserName(String::from("a"));

    let d1 = Domain::IntDomain(vec![Range::Bounded(1, 3)]);
    let d2 = Domain::IntDomain(vec![Range::Bounded(1, 2)]);

    let mut variables = BTreeMap::new();
    variables.insert(a.clone(), DecisionVariable { domain: d1.clone() });

    let mut m = Model::new(variables, vec![], Default::default());

    assert_eq!(m.variables.get(&a).unwrap().domain, d1);

    m.update_domain(&a, d2.clone());

    assert_eq!(m.variables.get(&a).unwrap().domain, d2);
}
