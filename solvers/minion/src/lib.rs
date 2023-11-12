//! This crate provides low level Rust bindings to the [Minion](https://github.com/minion/minion)
//! constraint solver.
//!
//! # Examples
//!
//! Consider the following Minion problem:
//!
//! ```plaintext
//! MINION 3
//! **VARIABLES**
//! DISCRETE x #
//! {1..3}
//! DISCRETE y #
//! {2..4}
//! DISCRETE z #
//! {1..5}
//! **SEARCH**
//! PRINT[[x],[y],[z]]
//! VARORDER STATIC [x, y, z]
//! **CONSTRAINTS**
//! sumleq([x,y,z],4)
//! ineq(x, y, -1)
//! **EOF**
//! ```
//!
//! This can be solved in Rust like so:
//!
//! ```
//! use minion_rs::ast::*;
//! use minion_rs::run_minion;
//! use std::collections::HashMap;
//!
//! // Get solutions out of Minion.
//! fn callback(solutions: HashMap<VarName, Constant>) -> bool {
//!     let x = match solutions.get("x").unwrap() {
//!         Constant::Integer(n) => n,
//!         _ => panic!("x should be a integer"),
//!     };
//!
//!     let y = match solutions.get("y").unwrap() {
//!         Constant::Integer(n) => n,
//!         _ => panic!("y should be a integer"),
//!     };
//!
//!     let z = match solutions.get("z").unwrap() {
//!         Constant::Integer(n) => n,
//!         _ => panic!("z should be a integer"),
//!     };
//!
//!     assert_eq!(*x, 1);
//!     assert_eq!(*y, 2);
//!     assert_eq!(*z, 1);
//!
//!     return true;
//! }
//!
//! // Build and run the model.
//! let mut model = Model::new();
//! model
//!     .named_variables
//!     .add_var("x".to_owned(), VarDomain::Bound(1, 3));
//! model
//!     .named_variables
//!     .add_var("y".to_owned(), VarDomain::Bound(2, 4));
//! model
//!     .named_variables
//!     .add_var("z".to_owned(), VarDomain::Bound(1, 5));
//!
//! let leq = Constraint::SumLeq(
//!     vec![
//!         Var::NameRef("x".to_owned()),
//!         Var::NameRef("y".to_owned()),
//!         Var::NameRef("z".to_owned()),
//!     ],
//!     Var::ConstantAsVar(4),
//! );
//!
//! let geq = Constraint::SumGeq(
//!     vec![
//!         Var::NameRef("x".to_owned()),
//!         Var::NameRef("y".to_owned()),
//!         Var::NameRef("z".to_owned()),
//!     ],
//!     Var::ConstantAsVar(4),
//! );
//!
//! let ineq = Constraint::Ineq(
//!     Var::NameRef("x".to_owned()),
//!     Var::NameRef("y".to_owned()),
//!     Constant::Integer(-1),
//! );
//!
//! model.constraints.push(leq);
//! model.constraints.push(geq);
//! model.constraints.push(ineq);
//!
//! let res = run_minion(model, callback);
//! res.expect("Error occurred");
//! ```

pub mod error;
mod raw_bindings;

mod run;
pub use run::*;

pub mod ast;

mod scoped_ptr;
