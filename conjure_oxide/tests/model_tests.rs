// Tests for various functionalities of the Model

use std::{cell::RefCell, rc::Rc};

use conjure_core::ast::Model;
use conjure_oxide::ast::*;
use declaration::Declaration;

#[test]
fn modify_domain() {
    let a = Name::UserName(String::from("a"));

    let d1 = Domain::IntDomain(vec![Range::Bounded(1, 3)]);
    let d2 = Domain::IntDomain(vec![Range::Bounded(1, 2)]);

    let mut symbols = SymbolTable::new();
    symbols
        .insert(Rc::new(Declaration::new_var(a.clone(), d1.clone())))
        .unwrap();

    let m = Model::new(Rc::new(RefCell::new(symbols)), vec![], Default::default());

    assert_eq!(&m.symbols().domain(&a).unwrap(), &d1);

    let mut decl_a = m.symbols().lookup(&a).unwrap();

    Rc::make_mut(&mut decl_a).as_var_mut().unwrap().domain = d2.clone();

    m.symbols_mut().update_insert(decl_a);

    assert_eq!(&m.symbols().domain(&a).unwrap(), &d2);
}
