use super::ParseContext;
use super::util::{get_expr_tree, query_toplevel};
use crate::diagnostics::source_map::SourceMap;
use crate::errors::FatalParseError;
use crate::expression::parse_expression;
use crate::util::TypecheckingContext;
use crate::util::node_is_expression;
use conjure_cp_core::ast::{Expression, SymbolTablePtr};
use std::collections::BTreeMap;
#[allow(unused)]
use uniplate::Uniplate;

pub fn parse_expr(
    src: &str,
    symbols_ptr: SymbolTablePtr,
) -> Result<Option<Expression>, FatalParseError> {
    let exprs = parse_exprs(src, symbols_ptr)?;
    if exprs.len() != 1 {
        return Ok(None);
    }
    Ok(Some(exprs[0].clone()))
}

pub fn parse_exprs(
    src: &str,
    symbols_ptr: SymbolTablePtr,
) -> Result<Vec<Expression>, FatalParseError> {
    let Some((tree, source_code)) = get_expr_tree(src) else {
        return Ok(Vec::new());
    };

    let root = tree.root_node();
    let mut source_map = SourceMap::default();
    let mut decl_spans = BTreeMap::new();
    let mut errors = Vec::new();
    let mut ctx = ParseContext::new(
        &source_code,
        &root,
        Some(symbols_ptr),
        &mut errors,
        &mut source_map,
        &mut decl_spans,
    );
    let mut ans = Vec::new();
    for expr in query_toplevel(&root, &node_is_expression) {
        ctx.typechecking_context = TypecheckingContext::Unknown;
        ctx.inner_typechecking_context = TypecheckingContext::Unknown;
        let Some(expr) = parse_expression(&mut ctx, expr)? else {
            continue;
        };
        ans.push(expr);
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
        Atom, DeclarationPtr, Domain, Expression, Literal, Metadata, Moo, Name, ReturnType,
        SymbolTable, Typeable,
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
            parse_expr("42", symbols.clone()).unwrap().unwrap(),
            Expression::Atomic(Metadata::new(), Atom::Literal(Literal::Int(42)))
        );
        assert_eq!(
            parse_expr("true", symbols.clone()).unwrap().unwrap(),
            Expression::Atomic(Metadata::new(), Atom::Literal(Literal::Bool(true)))
        );
        assert_eq!(
            parse_expr("false", symbols).unwrap().unwrap(),
            Expression::Atomic(Metadata::new(), Atom::Literal(Literal::Bool(false)))
        )
    }

    #[test]
    pub fn test_parse_expressions() {
        let src = "x >= 5, y = a / 2";
        let symbols = SymbolTablePtr::new();
        let x = DeclarationPtr::new_find(
            Name::User("x".into()),
            Domain::int(vec![conjure_cp_core::ast::Range::Bounded(0, 10)]),
        );

        let y = DeclarationPtr::new_find(
            Name::User("y".into()),
            Domain::int(vec![conjure_cp_core::ast::Range::Bounded(0, 10)]),
        );

        let a = DeclarationPtr::new_find(
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

    #[test]
    fn bars_distinguish_set_cardinality_from_integer_absolute_value() {
        let symbols = SymbolTablePtr::new();
        let set = DeclarationPtr::new_find(
            Name::User("s".into()),
            Domain::set(
                conjure_cp_core::ast::SetAttr::new_max_size(2),
                Domain::int(vec![conjure_cp_core::ast::Range::Bounded(1, 2)]),
            ),
        );
        let integer = DeclarationPtr::new_find(
            Name::User("x".into()),
            Domain::int(vec![conjure_cp_core::ast::Range::Bounded(-2, 2)]),
        );
        symbols.write().insert(set).unwrap();
        symbols.write().insert(integer).unwrap();

        let set_expr = parse_expr("|s| = 1", symbols.clone()).unwrap().unwrap();
        let Expression::Eq(_, set_left, _) = set_expr else {
            panic!("expected set cardinality comparison");
        };
        assert!(matches!(*set_left, Expression::Card(..)));

        let int_expr = parse_expr("|x| = 1", symbols).unwrap().unwrap();
        let Expression::Eq(_, int_left, _) = int_expr else {
            panic!("expected integer absolute-value comparison");
        };
        assert!(matches!(*int_left, Expression::Abs(..)));
    }

    #[test]
    pub fn test_parse_expression_annotations() {
        let symbols = SymbolTablePtr::new();
        let x = DeclarationPtr::new_find(
            Name::User("x".into()),
            Domain::int(vec![conjure_cp_core::ast::Range::Bounded(0, 10)]),
        );
        symbols
            .write()
            .insert(x.clone())
            .expect("x should not exist in the symbol-table yet, so we should be able to add it");

        let domain_annotation = parse_expr("x : int(1..3)", symbols.clone())
            .unwrap()
            .unwrap();
        assert!(matches!(
            domain_annotation,
            Expression::DomainAnnotation(_, _, _)
        ));
        assert_eq!(domain_annotation.to_string(), "x : int(1..3)");

        let type_annotation = parse_expr("x :: int", symbols.clone()).unwrap().unwrap();
        assert!(matches!(
            type_annotation,
            Expression::TypeAnnotation(_, _, _)
        ));
        assert_eq!(type_annotation.return_type(), ReturnType::Int);
        assert_eq!(type_annotation.to_string(), "x :: int");
    }

    #[test]
    pub fn test_parse_set_representation_preference() {
        let symbols = SymbolTablePtr::new();
        let x = DeclarationPtr::new_find(
            Name::User("x".into()),
            Domain::set(
                conjure_cp_core::ast::SetAttr::new_max_size(3),
                Domain::int(vec![conjure_cp_core::ast::Range::Bounded(1, 4)]),
            ),
        );
        symbols.write().insert(x).unwrap();

        let find_domain = parse_expr("x : set{packed} of int", symbols.clone())
            .unwrap()
            .unwrap();
        let Expression::DomainAnnotation(_, _, domain) = find_domain else {
            panic!("expected domain annotation");
        };
        assert_eq!(domain.representation_preference(), Some("packed"));
        assert_eq!(domain.to_string(), "set{packed} of int(-2147483647..2147483647)");

        let type_ann = parse_expr("x :: set{occurrence} of int", symbols.clone())
            .unwrap()
            .unwrap();
        assert_eq!(type_ann.to_string(), "x :: set{occurrence} of int");
        let Expression::TypeAnnotation(_, _, ty_domain) = type_ann else {
            panic!("expected type annotation");
        };
        assert_eq!(ty_domain.representation_preference(), Some("occurrence"));

        let nested = parse_expr(
            "x : set{explicit} of set{occurrence} of int",
            symbols,
        )
        .unwrap()
        .unwrap();
        let Expression::DomainAnnotation(_, _, nested_domain) = nested else {
            panic!("expected domain annotation");
        };
        assert_eq!(nested_domain.representation_preference(), Some("explicit"));
        let (_, inner) = nested_domain.as_set().unwrap();
        assert_eq!(inner.representation_preference(), Some("occurrence"));
        assert_eq!(
            nested_domain.to_string(),
            "set{explicit} of set{occurrence} of int(-2147483647..2147483647)"
        );
    }

    #[test]
    pub fn test_expression_annotations_bind_tighter_than_addition() {
        let symbols = SymbolTablePtr::new();
        let x = DeclarationPtr::new_find(
            Name::User("x".into()),
            Domain::int(vec![conjure_cp_core::ast::Range::Bounded(0, 10)]),
        );
        symbols
            .write()
            .insert(x)
            .expect("x should not exist in the symbol-table yet, so we should be able to add it");

        let expr = parse_expr("x + 1 : int", symbols).unwrap().unwrap();
        let Expression::Sum(_, terms) = expr else {
            panic!("expected a sum expression");
        };
        let terms = (*terms).clone().unwrap_list().unwrap();
        assert_eq!(terms.len(), 2);
        assert!(matches!(terms[1], Expression::DomainAnnotation(_, _, _)));
        assert_eq!(terms[1].to_string(), "1 : int(-2147483647..2147483647)");

        let symbols = SymbolTablePtr::new();
        let x = DeclarationPtr::new_find(
            Name::User("x".into()),
            Domain::int(vec![conjure_cp_core::ast::Range::Bounded(0, 10)]),
        );
        symbols
            .write()
            .insert(x)
            .expect("x should not exist in the symbol-table yet, so we should be able to add it");

        let expr = parse_expr("(x + 1) : int", symbols).unwrap().unwrap();
        let Expression::DomainAnnotation(_, inner, _) = expr else {
            panic!("expected a domain annotation");
        };
        assert!(matches!(*inner, Expression::Sum(_, _)));
    }

    #[test]
    pub fn test_parse_in_with_repr_annotation() {
        let symbols = SymbolTablePtr::new();
        let x = DeclarationPtr::new_find(
            Name::User("x".into()),
            Domain::set(
                conjure_cp_core::ast::SetAttr::new_max_size(3),
                Domain::int(vec![conjure_cp_core::ast::Range::Bounded(1, 4)]),
            ),
        );
        symbols.write().insert(x).unwrap();

        let expr = parse_expr("1 in x :: set{packed} of int", symbols.clone())
            .unwrap()
            .unwrap();
        println!("no parens: {expr}");
        assert!(
            matches!(expr, Expression::In(_, _, _)),
            "expected In, got {expr:?}"
        );
        let Expression::In(_, _, rhs) = &expr else {
            unreachable!()
        };
        assert!(
            matches!(rhs.as_ref(), Expression::TypeAnnotation(_, _, _)),
            "expected type annotation on in-rhs, got {rhs:?}"
        );

        let expr = parse_expr("1 in (x :: set{packed} of int)", symbols)
            .unwrap()
            .unwrap();
        println!("with parens: {expr}");
        let Expression::In(_, _, rhs) = expr else {
            panic!("expected In, got {expr:?}");
        };
        // Parentheses may wrap as Atomic-ish structure; accept TypeAnnotation directly or inside.
        let rhs_str = rhs.to_string();
        assert!(
            rhs_str.contains("set{packed}") || matches!(rhs.as_ref(), Expression::TypeAnnotation(_, _, _)),
            "expected annotated set on rhs, got {rhs:?}"
        );
    }
}