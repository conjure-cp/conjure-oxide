use conjure_core::ast::{Atom, Expression};
use conjure_core::matrix_expr;
use conjure_oxide::{Metadata, essence_expr};

#[test]
fn test_2plus2() {
    let expr = essence_expr!(2 + 2);
    assert_eq!(
        expr,
        Expression::Sum(
            Metadata::new(),
            Box::new(matrix_expr![
                Expression::Atomic(Metadata::new(), Atom::new_ilit(2)),
                Expression::Atomic(Metadata::new(), Atom::new_ilit(2))
            ])
        )
    );
}

// #[test]
// fn test_metavar_const() {
//     let x = 4;
//     let expr = essence_expr!(&x + 2);
//     assert_eq!(
//         expr,
//         Expression::Sum(
//             Metadata::new(),
//             Box::new(matrix_expr![
//                 Expression::Atomic(Metadata::new(), Atom::new_ilit(4)),
//                 Expression::Atomic(Metadata::new(), Atom::new_ilit(2))
//             ])
//         )
//     );
// }

// #[test]
// fn test_such_that() {
//     let expr = essence_expr!(x + 2 > y);
//     assert_eq!(
//         expr,
//         Expression::Gt(
//             Metadata::new(),
//             Box::new(Expression::Sum(
//                 Metadata::new(),
//                 Box::new(matrix_expr![
//                     Expression::Atomic(Metadata::new(), Atom::new_uref("x")),
//                     Expression::Atomic(Metadata::new(), Atom::new_ilit(2))
//                 ])
//             )),
//             Box::new(Expression::Atomic(Metadata::new(), Atom::new_uref("y")))
//         )
//     );
// }

// #[test]
// fn test_alldiff() {
//     let expr = essence_expr!(
//         // Comments work!
//         allDiff([a, b, c])
//     );
//     assert_eq!(
//         expr,
//         Expression::AllDiff(
//             Metadata::new(),
//             Box::new(matrix_expr![
//                 Expression::Atomic(Metadata::new(), Atom::new_uref("a")),
//                 Expression::Atomic(Metadata::new(), Atom::new_uref("b")),
//                 Expression::Atomic(Metadata::new(), Atom::new_uref("c")),
//             ])
//         )
//     )
// }

// #[test]
// fn test_and() {
//     let expr = essence_expr!("a /\\ b");
//     assert_eq!(
//         expr,
//         Expression::And(
//             Metadata::new(),
//             Box::new(matrix_expr![
//                 Expression::Atomic(Metadata::new(), Atom::new_uref("a")),
//                 Expression::Atomic(Metadata::new(), Atom::new_uref("b")),
//             ])
//         )
//     )
// }

// #[test]
// fn text_leq_meta() {
//     let meta = essence_expr!(a + 2);
//     let expr = essence_expr!(x <= &meta);
//     assert_eq!(
//         expr,
//         Expression::Leq(
//             Metadata::new(),
//             Box::new(Expression::Atomic(Metadata::new(), Atom::new_uref("x"))),
//             Box::new(Expression::Sum(
//                 Metadata::new(),
//                 Box::new(matrix_expr![
//                     Expression::Atomic(Metadata::new(), Atom::new_uref("a")),
//                     Expression::Atomic(Metadata::new(), Atom::new_ilit(2))
//                 ])
//             )),
//         )
//     );
// }

// #[test]
// fn test_essence_vec_basic() {
//     let exprs = essence_vec!(a + 2, b = true);
//     assert_eq!(exprs.len(), 2);
//     assert_eq!(
//         exprs[0],
//         Expression::Sum(
//             Metadata::new(),
//             Box::new(matrix_expr![
//                 Expression::Atomic(Metadata::new(), Atom::new_uref("a")),
//                 Expression::Atomic(Metadata::new(), Atom::new_ilit(2))
//             ])
//         )
//     );
//     assert_eq!(
//         exprs[1],
//         Expression::Eq(
//             Metadata::new(),
//             Box::new(Expression::Atomic(Metadata::new(), Atom::new_uref("b"))),
//             Box::new(Expression::Atomic(Metadata::new(), Atom::new_blit(true)))
//         )
//     );
// }

// #[test]
// fn test_essence_vec() {
//     let exprs = essence_vec!(r"(x /\ y) -> true, |a / |b|| = 42");
//     assert_eq!(exprs.len(), 2);
//     assert_eq!(
//         exprs[0],
//         Expression::Imply(
//             Metadata::new(),
//             Box::new(Expression::And(
//                 Metadata::new(),
//                 Box::new(matrix_expr![
//                     Expression::Atomic(Metadata::new(), Atom::new_uref("x")),
//                     Expression::Atomic(Metadata::new(), Atom::new_uref("y")),
//                 ])
//             )),
//             Box::new(Expression::Atomic(Metadata::new(), Atom::new_blit(true)))
//         )
//     );
//     assert_eq!(
//         exprs[1],
//         Expression::Eq(
//             Metadata::new(),
//             Box::new(Expression::Abs(
//                 Metadata::new(),
//                 Box::new(Expression::UnsafeDiv(
//                     Metadata::new(),
//                     Box::new(Expression::Atomic(Metadata::new(), Atom::new_uref("a"))),
//                     Box::new(Expression::Abs(
//                         Metadata::new(),
//                         Box::new(Expression::Atomic(Metadata::new(), Atom::new_uref("b")))
//                     ))
//                 ))
//             )),
//             Box::new(Expression::Atomic(Metadata::new(), Atom::new_ilit(42)))
//         )
//     )
// }
