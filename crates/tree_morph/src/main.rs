// This file is for testing purposes only.
// It is not meant to be integrated into the main code.

// use std::cell::Cell;

// use tree_morph::prelude::*;
// use uniplate::derive::Uniplate;

// static mut GLOBAL_RULE_CHECKS: Cell<usize> = Cell::new(0);

// #[derive(Debug, Clone, PartialEq, Eq, Uniplate)]
// #[uniplate()]
// enum Expr {
//     Add(Box<Expr>, Box<Expr>),
//     Mul(Box<Expr>, Box<Expr>),
//     Sub(Box<Expr>, Box<Expr>),
//     Val(i32),
// }

// fn rule_eval_add(_: &mut Commands<Expr, Meta>, expr: &Expr, _: &Meta) -> Option<Expr> {
//     dbg!(expr);
//     match expr {
//         Expr::Add(a, b) => match (a.as_ref(), b.as_ref()) {
//             (Expr::Val(x), Expr::Val(y)) => Some(Expr::Val(x + y)),
//             _ => None,
//         },
//         _ => None,
//     }
// }

// fn rule_eval_mul(_: &mut Commands<Expr, Meta>, expr: &Expr, _: &Meta) -> Option<Expr> {
//     dbg!(expr);
//     match expr {
//         Expr::Mul(a, b) => match (a.as_ref(), b.as_ref()) {
//             (Expr::Val(x), Expr::Val(y)) => Some(Expr::Val(x * y)),
//             _ => None,
//         },
//         _ => None,
//     }
// }

// enum MyRule {
//     EvalAdd,
//     EvalMul,
// }

// struct Meta {
//     num_applications: u32,
//     num_checks: u32,
// }

// impl Rule<Expr, Meta> for MyRule {
//     fn apply(&self, cmd: &mut Commands<Expr, Meta>, expr: &Expr, meta: &Meta) -> Option<Expr> {
//         println!("Trying Rule");
//         cmd.mut_meta(Box::new(|m: &mut Meta| m.num_applications += 1)); // Only applied if successful
//                                                                         // THIS IS FOR TESTING ONLY
//                                                                         // Not meant to integrated into the main code.
//         unsafe {
//             GLOBAL_RULE_CHECKS.set(GLOBAL_RULE_CHECKS.get() + 1);
//         }
//         match self {
//             MyRule::EvalAdd => rule_eval_add(cmd, expr, meta),
//             MyRule::EvalMul => rule_eval_mul(cmd, expr, meta),
//         }
//     }
// }

// fn left_branch_clean() {
//     // Top Level is +
//     // Left Branch has two Nested Subtractions which do not have any rules
//     // So atoms
//     // Right brigh has Mul and Add which DO have rules
//     let expr = Expr::Sub(
//         // Box::new(Expr::Val(1)),
//         Box::new(Expr::Sub(
//             Box::new(Expr::Val(1)),
//             Box::new(Expr::Val(1)),
//             // Box::new(Expr::Sub(Box::new(Expr::Val(1)), Box::new(Expr::Val(2)))),
//             // Box::new(Expr::Sub(Box::new(Expr::Val(3)), Box::new(Expr::Val(10)))),
//         )),
//         Box::new(Expr::Mul(Box::new(Expr::Val(10)), Box::new(Expr::Val(5)))),
//     );

//     let meta = Meta {
//         num_applications: 0,
//         num_checks: 0,
//     };

//     println!("{:?}", expr);
//     let (expr, meta) = morph(
//         vec![vec![MyRule::EvalAdd, MyRule::EvalMul]],
//         select_first,
//         expr,
//         meta,
//     );

//     println!("RAN TESTS");
//     println!("Number of applications: {}", meta.num_applications);
//     unsafe {
//         println!(
//             "Number of Rule Application Checks {}",
//             GLOBAL_RULE_CHECKS.get()
//         );
//     }
//     println!("{:?}", expr);
// }
fn main() {}
