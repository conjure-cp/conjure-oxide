use super::util::{get_tree, query_toplevel};
use crate::errors::EssenceParseError;
use crate::expression::parse_expression;
use conjure_cp_core::ast::{Expression, SymbolTable};
#[allow(unused)]
use uniplate::Uniplate;

pub fn parse_expr(src: &str, symbol_table: &SymbolTable) -> Result<Expression, EssenceParseError> {
    let exprs = parse_exprs(src, symbol_table)?;
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
    symbol_table: &SymbolTable,
) -> Result<Vec<Expression>, EssenceParseError> {
    let (tree, source_code) = get_tree(src).ok_or(EssenceParseError::TreeSitterError(
        "Failed to parse Essence source code".to_string(),
    ))?;

    let root = tree.root_node();
    let mut ans = Vec::new();
    for expr in query_toplevel(&root, &|n| n.kind() == "expression") {
        ans.push(parse_expression(
            expr,
            &source_code,
            &root,
            Some(symbol_table),
        )?);
    }
    Ok(ans)
}

mod test {
    #[allow(unused)]
    use super::{parse_expr, parse_exprs};
    #[allow(unused_imports)]
    use conjure_cp_core::ast::Moo;
    #[allow(unused)]
    use conjure_cp_core::ast::{DeclarationPtr, Domain, Name, SymbolTable};
    #[allow(unused)]
    use conjure_cp_core::{ast::Atom, ast::Expression, ast::Metadata};
    #[allow(unused)]
    use std::collections::HashMap;
    #[allow(unused)]
    use std::sync::Arc;
    #[allow(unused)]
    use std::{cell::RefCell, rc::Rc};
    #[allow(unused)]
    use tree_sitter::Range;

    #[test]
    pub fn test_parse_expressions() {
        let src = "x >= 5, y = a / 2";
        let mut symbols = SymbolTable::new();
        let x = DeclarationPtr::new_var(
            Name::User("x".into()),
            Domain::Int(vec![conjure_cp_core::ast::Range::Bounded(0, 10)]),
        );

        let y = DeclarationPtr::new_var(
            Name::User("y".into()),
            Domain::Int(vec![conjure_cp_core::ast::Range::Bounded(0, 10)]),
        );

        let a = DeclarationPtr::new_var(
            Name::User("a".into()),
            Domain::Int(vec![conjure_cp_core::ast::Range::Bounded(0, 10)]),
        );

        // Clone the Rc when inserting!
        symbols
            .insert(x.clone())
            .expect("x should not exist in the symbol-table yet, so we should be able to add it");

        symbols
            .insert(y.clone())
            .expect("y should not exist in the symbol-table yet, so we should be able to add it");

        symbols
            .insert(a.clone())
            .expect("a should not exist in the symbol-table yet, so we should be able to add it");

        let exprs = parse_exprs(src, &symbols).unwrap();
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
