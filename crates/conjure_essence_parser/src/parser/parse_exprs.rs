use conjure_core::ast::Expression;
use conjure_core::error::Error;
#[allow(unused)]
use uniplate::Uniplate;

use crate::errors::EssenceParseError;

use super::expression::parse_expression;
use super::util::{get_tree, query_toplevel};

pub fn parse_expr(src: &str) -> Result<Expression, EssenceParseError> {
    let exprs = parse_exprs(src)?;
    if exprs.len() != 1 {
        return Err(EssenceParseError::ParseError(Error::Parse(
            "Expected exactly one expression".into(),
        )));
    }
    Ok(exprs[0].clone())
}

pub fn parse_exprs(src: &str) -> Result<Vec<Expression>, EssenceParseError> {
    let (tree, source_code) = get_tree(src).ok_or(EssenceParseError::TreeSitterError(
        "Failed to parse Essence source code".to_string(),
    ))?;

    let root = tree.root_node();
    let mut ans = Vec::new();
    for expr in query_toplevel(&root, &|n| n.kind() == "expression") {
        ans.push(parse_expression(expr, &source_code, &root)?);
    }
    Ok(ans)
}

mod test {
    #[allow(unused)]
    use super::{parse_expr, parse_exprs};
    #[allow(unused)]
    use conjure_core::{ast::Atom, ast::Expression, metadata::Metadata};
    #[allow(unused)]
    use std::collections::HashMap;
    #[allow(unused)]
    use std::sync::Arc;

    #[test]
    pub fn test_parse_expressions() {
        let src = "x >= 5, y = a / 2";
        let exprs = parse_exprs(src).unwrap();
        assert_eq!(exprs.len(), 2);
        assert_eq!(
            exprs[0],
            Expression::Geq(
                Metadata::new(),
                Box::new(Expression::Atomic(Metadata::new(), Atom::new_uref("x"))),
                Box::new(Expression::Atomic(Metadata::new(), 5.into()))
            )
        );
        assert_eq!(
            exprs[1],
            Expression::Eq(
                Metadata::new(),
                Box::new(Expression::Atomic(Metadata::new(), Atom::new_uref("y"))),
                Box::new(Expression::UnsafeDiv(
                    Metadata::new(),
                    Box::new(Expression::Atomic(Metadata::new(), Atom::new_uref("a"))),
                    Box::new(Expression::Atomic(Metadata::new(), 2.into()))
                ))
            )
        );
    }
}
