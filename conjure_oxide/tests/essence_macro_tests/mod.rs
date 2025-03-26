use conjure_core::ast::{Atom, Expression, Literal, Name};
use conjure_oxide::Metadata;
use essence_macros::essence_expr;

#[test]
pub fn test_essence_basic() {
    let expr = essence_expr!("x + 2");
    assert_eq!(
        expr,
        Expression::Sum(
            Metadata::new(),
            vec![
                Expression::Atomic(Metadata::new(), Atom::new_uref("x")),
                Expression::Atomic(Metadata::new(), Atom::new_ilit(2))
            ]
        )
    )
}
