// Tests for various functionalities of the Model

use conjure_cp::ast::Model;
use conjure_cp::ast::*;
use conjure_cp::range;
use conjure_cp_core::domain_int;

#[test]
fn modify_domain() {
    let mut m = Model::new(Default::default());

    let mut symbols = m.as_submodel_mut().symbols_mut();

    let name_a = Name::user("a");

    let d1 = Domain::int(vec![Range::Bounded(1, 3)]);
    let d2 = Domain::int(vec![Range::Bounded(1, 2)]);

    let mut decl_a = DeclarationPtr::new_var(name_a, d1.clone());

    symbols.insert(decl_a.clone()).unwrap();

    assert_eq!(&decl_a.domain().unwrap(), &d1);

    decl_a.as_var_mut().unwrap().domain = d2.clone();

    assert_eq!(&decl_a.domain().unwrap(), &d2);
}

#[test]
fn assignment_ok() {
    let mut m = Model::new(Default::default());

    let x = DeclarationPtr::new_var(Name::user("x"), Domain::bool());
    let y = DeclarationPtr::new_var(Name::user("y"), domain_int!(1..3, 5));

    {
        let mut symbols = m.as_submodel_mut().symbols_mut();
        [&x, &y].into_iter().for_each(|v| {
            symbols
                .insert(v.clone())
                .unwrap_or_else(|| panic!("could not insert {}", v.name()))
        });
    }

    let ab = m.as_submodel().new_assignment();
    let res = ab
        .insert(x, true.into())
        .and_then(|ab| ab.insert(y, 2.into()))
        .and_then(AssignmentBuilder::build);
    assert!(res.is_ok());
}
