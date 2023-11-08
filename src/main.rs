use std::{collections::HashMap, fs::File, io::Read};

use conjure_oxide::ast::Model;
use conjure_oxide::parse::*;

fn main() {
    let mut abc = File::open("examples/abc.json").unwrap();
    let mut abc_str = String::new();
    abc.read_to_string(&mut abc_str).unwrap();

    let m = Model::from_json(&abc_str);

    // find a,b,c : int(1..3)
    // such that a + b + c = 4
    // such that a >= b

    // let a = Name::UserName(String::from("a"));
    // let b = Name::UserName(String::from("b"));
    // let c = Name::UserName(String::from("c"));

    // let mut variables = HashMap::new();
    // variables.insert(
    //     a.clone(),
    //     DecisionVariable {
    //         domain: Domain::IntDomain(vec![Range::Bounded(1, 3)]),
    //     },
    // );
    // variables.insert(
    //     b.clone(),
    //     DecisionVariable {
    //         domain: Domain::IntDomain(vec![Range::Bounded(1, 3)]),
    //     },
    // );
    // variables.insert(
    //     c.clone(),
    //     DecisionVariable {
    //         domain: Domain::IntDomain(vec![Range::Bounded(1, 3)]),
    //     },
    // );

    // let mut m = Model {
    //     variables,
    //     constraints: vec![
    //         Expression::Eq(
    //             Box::new(Expression::Sum(vec![
    //                 Expression::Reference(a.clone()),
    //                 Expression::Reference(b.clone()),
    //                 Expression::Reference(c.clone()),
    //             ])),
    //             Box::new(Expression::ConstantInt(4)),
    //         ),
    //         Expression::Geq(
    //             Box::new(Expression::Reference(a.clone())),
    //             Box::new(Expression::Reference(b.clone())),
    //         ),
    //     ],
    // };
}
