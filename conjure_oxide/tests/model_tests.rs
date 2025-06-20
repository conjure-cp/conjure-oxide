// Tests for various functionalities of the Model

use std::rc::Rc;

use conjure_core::ast::Model;
use conjure_oxide::ast::*;

#[test]
fn modify_domain() {
    let mut m = Model::new(Default::default());

    let mut symbols = m.as_submodel_mut().symbols_mut();

    let a = Name::UserName(String::from("a"));

    let d1 = Domain::Int(vec![Range::Bounded(1, 3)]);
    let d2 = Domain::Int(vec![Range::Bounded(1, 2)]);

    symbols
        .insert(Rc::new(Declaration::new_var(a.clone(), d1.clone())))
        .unwrap();

    assert_eq!(symbols.domain(&a).unwrap(), d1);

    let mut decl_a = symbols.lookup(&a).unwrap();

    Rc::make_mut(&mut decl_a).as_var_mut().unwrap().domain = d2.clone();

    symbols.update_insert(decl_a);

    assert_eq!(symbols.domain(&a).unwrap(), d2);
}
