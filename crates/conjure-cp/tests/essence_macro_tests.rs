use conjure_cp::ast::Metadata;
use conjure_cp::ast::{Atom, Expression, Moo};
use conjure_cp::essence_expr;
use conjure_cp::matrix_expr;

#[test]
fn test_2plus2() {
    let expr = essence_expr!(2 + 2);
    assert_eq!(
        expr,
        Expression::Sum(
            Metadata::new(),
            Moo::new(matrix_expr![
                Expression::Atomic(Metadata::new(), 2.into()),
                Expression::Atomic(Metadata::new(), 2.into())
            ])
        )
    );
}

#[test]
fn test_metavar_const() {
    let x = 4;
    let expr = essence_expr!(&x + 2);
    assert_eq!(
        expr,
        Expression::Sum(
            Metadata::new(),
            Moo::new(matrix_expr![
                Expression::Atomic(Metadata::new(), 4.into()),
                Expression::Atomic(Metadata::new(), 2.into())
            ])
        )
    );
}

// TODO (gs248): We need to be able to generate Atom::Reference via the essence_expr! macro.
// These used to just be a string (variable name). Now, they are a pointer to Declaration.
// Thus we need to be able to pass the symbol table into the macro when working with references.
// Needs design + implementation.
//
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
//                     Expression::Atomic(Metadata::new(), 2.into())
//                 ])
//             )),
//             Box::new(Expression::Atomic(Metadata::new(), Atom::new_uref("y")))
//         )
//     );
// }
