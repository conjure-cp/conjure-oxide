// Tests for various functionalities of the Model

use conjure_cp::ast::Model;
use conjure_cp::ast::*;

#[test]
fn modify_domain() {
    let mut m = Model::new(Default::default());

    let mut symbols = m.as_submodel_mut().symbols_mut();

    let name_a = Name::user("a");

    let d1 = Domain::Int(vec![Range::Bounded(1, 3)]);
    let d2 = Domain::Int(vec![Range::Bounded(1, 2)]);

    let mut decl_a = DeclarationPtr::new_var(name_a, d1.clone());

    symbols.insert(decl_a.clone()).unwrap();

    assert_eq!(&decl_a.domain().unwrap() as &Domain, &d1);

    decl_a.as_var_mut().unwrap().domain = d2.clone();

    assert_eq!(&decl_a.domain().unwrap() as &Domain, &d2);
}
