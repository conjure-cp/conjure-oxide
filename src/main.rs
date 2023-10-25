use std::{cell::RefCell, rc::Rc};

mod common;
use common::*;

fn main() {
    let a = ast::Name::UserName(String::from("a"));
    let b = ast::Name::UserName(String::from("b"));
    let c = ast::Name::UserName(String::from("c"));

    let a_decision_variable = Rc::new(RefCell::new(ast::DecisionVariable {
        name: a,
        domain: ast::Domain::IntDomain(vec![ast::Range::Bounded(1, 3)]),
    }));
    let b_decision_variable = Rc::new(RefCell::new(ast::DecisionVariable {
        name: b,
        domain: ast::Domain::IntDomain(vec![ast::Range::Bounded(1, 3)]),
    }));
    let c_decision_variable = Rc::new(RefCell::new(ast::DecisionVariable {
        name: c,
        domain: ast::Domain::IntDomain(vec![ast::Range::Bounded(1, 3)]),
    }));

    // find a,b,c : int(1..3)
    // such that a + b + c = 4
    // such that a >= b
    let m = ast::Model {
        statements: vec![
            ast::Statement::Declaration(Rc::clone(&a_decision_variable)),
            ast::Statement::Declaration(Rc::clone(&b_decision_variable)),
            ast::Statement::Declaration(Rc::clone(&c_decision_variable)),
            ast::Statement::Constraint(ast::Expression::Eq(
                Box::from(ast::Expression::Sum(vec![
                    ast::Expression::Reference(Rc::clone(&a_decision_variable)),
                    ast::Expression::Reference(Rc::clone(&b_decision_variable)),
                    ast::Expression::Reference(Rc::clone(&c_decision_variable)),
                ])),
                Box::from(ast::Expression::ConstantInt(4)),
            )),
            ast::Statement::Constraint(ast::Expression::Geq(
                Box::from(ast::Expression::Reference(Rc::clone(&a_decision_variable))),
                Box::from(ast::Expression::Reference(Rc::clone(&b_decision_variable))),
            )),
        ],
    };

    println!("{:#?}", m);

    {
        let mut decision_var_borrowed = a_decision_variable.borrow_mut();
        decision_var_borrowed.domain = ast::Domain::IntDomain(vec![ast::Range::Bounded(1, 2)]);
    }

    println!("{:#?}", m);
}
