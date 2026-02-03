use super::util::{get_tree, query_toplevel};
use crate::errors::EssenceParseError;
use crate::expression::parse_expression;
use crate::util::node_is_expression;
use conjure_cp_core::ast::{Expression, SymbolTablePtr};
#[allow(unused)]
use uniplate::Uniplate;

pub fn parse_expr(src: &str, symbols_ptr: SymbolTablePtr) -> Result<Expression, EssenceParseError> {
    let exprs = parse_exprs(src, symbols_ptr)?;
    if exprs.len() != 1 {
        return Err(EssenceParseError::syntax_error(
            "Expected a single expression".to_string(),
            None,
        ));
    }
    Ok(exprs[0].clone())
}

pub fn parse_exprs(
    src: &str,
    symbols_ptr: SymbolTablePtr,
) -> Result<Vec<Expression>, EssenceParseError> {
    let (tree, source_code) = get_tree(src).ok_or(EssenceParseError::TreeSitterError(
        "Failed to parse Essence source code".to_string(),
    ))?;

    let root = tree.root_node();
    let mut ans = Vec::new();
    for expr in query_toplevel(&root, &node_is_expression) {
        ans.push(parse_expression(
            expr,
            &source_code,
            &root,
            Some(symbols_ptr.clone()),
        )?);
    }
    Ok(ans)
}

mod test {
    #[allow(unused)]
    use super::{parse_expr, parse_exprs};
    #[allow(unused)]
    use conjure_cp_core::ast::SymbolTablePtr;
    #[allow(unused)]
    use conjure_cp_core::ast::{
        Atom, DeclarationPtr, Domain, Expression, Literal, Metadata, Moo, Name, SymbolTable,
    };
    #[allow(unused)]
    use std::collections::HashMap;
    #[allow(unused)]
    use std::sync::Arc;
    #[allow(unused)]
    use tree_sitter::Range;

    #[test]
    pub fn test_parse_constant() {
        let symbols = SymbolTablePtr::new();

        assert_eq!(
            parse_expr("42", symbols.clone()).unwrap(),
            Expression::Atomic(Metadata::new(), Atom::Literal(Literal::Int(42)))
        );
        assert_eq!(
            parse_expr("true", symbols.clone()).unwrap(),
            Expression::Atomic(Metadata::new(), Atom::Literal(Literal::Bool(true)))
        );
        assert_eq!(
            parse_expr("false", symbols).unwrap(),
            Expression::Atomic(Metadata::new(), Atom::Literal(Literal::Bool(false)))
        )
    }

    #[test]
    pub fn test_parse_expressions() {
        let src = "x >= 5, y = a / 2";
        let symbols = SymbolTablePtr::new();
        let x = DeclarationPtr::new_var(
            Name::User("x".into()),
            Domain::int(vec![conjure_cp_core::ast::Range::Bounded(0, 10)]),
        );

        let y = DeclarationPtr::new_var(
            Name::User("y".into()),
            Domain::int(vec![conjure_cp_core::ast::Range::Bounded(0, 10)]),
        );

        let a = DeclarationPtr::new_var(
            Name::User("a".into()),
            Domain::int(vec![conjure_cp_core::ast::Range::Bounded(0, 10)]),
        );

        // Clone the Rc when inserting!
        symbols
            .write()
            .insert(x.clone())
            .expect("x should not exist in the symbol-table yet, so we should be able to add it");

        symbols
            .write()
            .insert(y.clone())
            .expect("y should not exist in the symbol-table yet, so we should be able to add it");

        symbols
            .write()
            .insert(a.clone())
            .expect("a should not exist in the symbol-table yet, so we should be able to add it");

        let exprs = parse_exprs(src, symbols).unwrap();
        assert_eq!(exprs.len(), 2);

        assert_eq!(
            exprs[0],
            Expression::Geq(
                Metadata::new(),
                Moo::new(Expression::Atomic(Metadata::new(), Atom::new_ref(x))),
                Moo::new(Expression::Atomic(Metadata::new(), 5.into()))
            )
        );

        assert_eq!(
            exprs[1],
            Expression::Eq(
                Metadata::new(),
                Moo::new(Expression::Atomic(Metadata::new(), Atom::new_ref(y))),
                Moo::new(Expression::UnsafeDiv(
                    Metadata::new(),
                    Moo::new(Expression::Atomic(Metadata::new(), Atom::new_ref(a))),
                    Moo::new(Expression::Atomic(Metadata::new(), 2.into()))
                ))
            )
        );
    }
}
