use conjure_cp::ast::Metadata;
use conjure_cp::ast::{Expression, Moo};
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

#[test]
fn test_gt() {
    let x = essence_expr!(2);
    let expr = essence_expr!(&x + 2 > 3);
    assert_eq!(
        expr,
        Expression::Gt(
            Metadata::new(),
            Moo::new(Expression::Sum(
                Metadata::new(),
                Moo::new(matrix_expr![
                    x,
                    Expression::Atomic(Metadata::new(), 2.into())
                ])
            )),
            Moo::new(Expression::Atomic(Metadata::new(), 3.into()))
        )
    );
}
