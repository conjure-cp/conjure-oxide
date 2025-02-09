// Tests for various functionalities of the Model

use conjure_core::ast::Model;
use conjure_oxide::ast::*;

#[test]
fn modify_domain() {
    let a = Name::UserName(String::from("a"));

    let d1 = Domain::IntDomain(vec![Range::Bounded(1, 3)]);
    let d2 = Domain::IntDomain(vec![Range::Bounded(1, 2)]);

    let mut symbols = SymbolTable::new();
    symbols.add_var(a.clone(), DecisionVariable { domain: d1.clone() });

    let mut m = Model::new(symbols, vec![], Default::default());

    assert_eq!(m.symbols().domain_of(&a).unwrap(), &d1);

    *m.symbols_mut().domain_of_mut(&a).unwrap() = d2.clone();

    assert_eq!(m.symbols().domain_of(&a).unwrap(), &d2);
}
