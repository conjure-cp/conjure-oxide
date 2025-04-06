use conjure_core::ast::{Atom, Expression};
use conjure_core::matrix_expr;
use conjure_oxide::{essence_expr, Metadata};

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

#[test]
fn test_metavar_const() {
    let x = 4;
    let expr = essence_expr!(&x + 2);
    assert_eq!(
        expr,
        Expression::Sum(
            Metadata::new(),
            Box::new(matrix_expr![
                Expression::Atomic(Metadata::new(), Atom::new_ilit(4)),
                Expression::Atomic(Metadata::new(), Atom::new_ilit(2))
            ])
        )
    );
}

#[test]
fn test_such_that() {
    let expr = essence_expr!(such that x + 2 > y);
    assert_eq!(
        expr,
        Expression::Gt(
            Metadata::new(),
            Box::new(Expression::Sum(
                Metadata::new(),
                Box::new(matrix_expr![
                    Expression::Atomic(Metadata::new(), Atom::new_uref("x")),
                    Expression::Atomic(Metadata::new(), Atom::new_ilit(2))
                ])
            )),
            Box::new(Expression::Atomic(Metadata::new(), Atom::new_uref("y")))
        )
    );
}

#[test]
fn test_alldiff() {
    let expr = essence_expr!(allDiff([a, b, c]));
    assert_eq!(
        expr,
        Expression::AllDiff(
            Metadata::new(),
            Box::new(matrix_expr![
                Expression::Atomic(Metadata::new(), Atom::new_uref("a")),
                Expression::Atomic(Metadata::new(), Atom::new_uref("b")),
                Expression::Atomic(Metadata::new(), Atom::new_uref("c")),
            ])
        )
    )
}
