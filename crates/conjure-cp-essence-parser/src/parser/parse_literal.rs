use crate::errors::{ConjureParseError, EssenceParseError};
use crate::parse_expr;
use conjure_cp_core::ast::{Atom, Expression, Literal, SymbolTable};
#[allow(unused)]
use uniplate::Uniplate;

pub fn parse_literal(src: &str) -> Result<Literal, EssenceParseError> {
    // HACK: Create a dummy symbol table - don't need it for parsing literals
    let symbol_table = SymbolTable::new();
    let expr = parse_expr(src, &symbol_table)?;
    match expr {
        Expression::Atomic(_metadata, atom) => match atom {
            Atom::Literal(lit) => Ok(lit),
            _ => Err(ConjureParseError::Parse(format!("Expected a literal, got '{atom}'")).into()),
        },
        _ => Err(ConjureParseError::Parse(format!("Expected a literal, got '{expr}'")).into()),
    }
}

mod test {
    #[allow(unused)]
    use super::parse_literal;
    #[allow(unused)]
    use conjure_cp_core::ast::Metadata;
    #[allow(unused)]
    use conjure_cp_core::ast::{
        Atom, DeclarationPtr, Domain, Expression, Literal, Moo, Name, SymbolTable,
    };
    #[allow(unused)]
    use std::collections::HashMap;
    #[allow(unused)]
    use std::sync::Arc;
    #[allow(unused)]
    use std::{cell::RefCell, rc::Rc};
    #[allow(unused)]
    use tree_sitter::Range;

    #[test]
    pub fn test_parse_bool() {
        let src_true = "true";
        let src_false = "false";
        let literal_true = parse_literal(src_true).unwrap();
        let literal_false = parse_literal(src_false).unwrap();
        assert_eq!(literal_true, Literal::Bool(true));
        assert_eq!(literal_false, Literal::Bool(false));
    }

    #[test]
    pub fn test_parse_int() {
        let src_int = "42";
        let literal_int = parse_literal(src_int).unwrap();
        assert_eq!(literal_int, Literal::Int(42));
    }

    #[test]
    pub fn test_parse_neg_int() {
        let src_int = "-42";
        let literal_int = parse_literal(src_int).unwrap();
        assert_eq!(literal_int, Literal::Int(-42));
    }

    #[test]
    pub fn test_bad() {
        let src_bad = "bad";
        let src_expr = "2 + 2";
        let literal_bad = parse_literal(src_bad);
        let literal_expr = parse_literal(src_expr);
        assert!(literal_bad.is_err());
        assert!(literal_expr.is_err());
    }
}
