use std::collections::HashMap;
use std::rc::Rc;
use std::sync::Arc;

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
    let (tree, source_code) = match get_tree(src) {
        Some(t) => t,
        None => {
            return Err(EssenceParseError::TreeSitterError(
                "Failed to parse source code".to_string(),
            ))
        }
    };

    let root = tree.root_node();
    let mut ans = Vec::new();
    for expr in query_toplevel(&root, &|n| n.kind() == "expression") {
        ans.push(parse_expression(expr, &source_code, &root));
    }
    Ok(ans)
}

fn replace_metavars(expr: &Expression, metavars: Rc<HashMap<String, Expression>>) -> Expression {
    expr.rewrite(Rc::new(move |sub| match &sub {
        Expression::Metavar(_, name) => metavars.get(name).cloned(),
        _ => None,
    }))
}

pub fn parse_expr_with_metavars(
    src: &str,
    metavars: Rc<HashMap<String, Expression>>,
) -> Result<Expression, EssenceParseError> {
    Ok(replace_metavars(&parse_expr(src)?, metavars))
}

pub fn parse_exprs_with_metavars(
    src: &str,
    metavars: Rc<HashMap<String, Expression>>,
) -> Result<Vec<Expression>, EssenceParseError> {
    Ok(parse_exprs(src)?
        .iter()
        .map(|expr| replace_metavars(expr, metavars.clone()))
        .collect())
}

mod test {
    #[allow(unused)]
    use super::{parse_expr, parse_expr_with_metavars, parse_exprs, parse_exprs_with_metavars};
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

    #[test]
    pub fn test_parse_expression_with_metavars() {
        let src = "y = &expr / 2";
        let expr = parse_expr("z + 5").unwrap();

        let mut metas: HashMap<String, Expression> = HashMap::new();
        metas.insert("expr".into(), expr);

        let expr = parse_expr_with_metavars(src, Arc::new(metas)).unwrap();
        assert_eq!(
            expr,
            Expression::Eq(
                Metadata::new(),
                Box::new(Expression::Atomic(Metadata::new(), Atom::new_uref("y"))),
                Box::new(Expression::UnsafeDiv(
                    Metadata::new(),
                    Box::new(Expression::Sum(
                        Metadata::new(),
                        vec![
                            Expression::Atomic(Metadata::new(), Atom::new_uref("z")),
                            Expression::Atomic(Metadata::new(), Atom::new_ilit(5))
                        ]
                    )),
                    Box::new(Expression::Atomic(Metadata::new(), Atom::new_ilit(2)))
                ))
            )
        );
    }
}
