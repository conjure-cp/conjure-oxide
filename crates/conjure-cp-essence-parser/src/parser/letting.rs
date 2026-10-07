#![allow(clippy::legacy_numeric_constants)]
use crate::field;
use std::collections::BTreeSet;
use tree_sitter::Node;

use super::ParseContext;
use super::domain::parse_domain;
use super::keyword_checks::is_keyword_identifier;
use super::util::named_children;
use crate::diagnostics::diagnostics_api::SymbolKind;
use crate::diagnostics::source_map::{HoverInfo, span_with_hover};
use crate::errors::{FatalParseError, RecoverableParseError};
use crate::expression::parse_expression;
use conjure_cp_core::ast::DeclarationPtr;
use conjure_cp_core::ast::{
    Domain, DomainPtr, Expression, IntVal, Literal, Moo, Name, Range, ReturnType, SymbolTable,
    Typeable, eval_constant,
};

/// Infer and retain the domain of a value letting when it enters the symbol table.
///
/// Integer lettings denote a single (possibly parameter-dependent) value:
///
/// - Constant integers become a ground singleton so dependent domains such as
///   `int(1..10**n)` stay tight after the letting is installed.
/// - Non-constant integers become a symbolic [`IntVal::Expr`] singleton. This remains
///   resolvable after referenced `given` declarations are instantiated and, unlike eager
///   interval evaluation, also works when those declarations currently have unbounded or
///   full-width (`int`) domains.
///
/// Eager `Expression::domain_of` must not run here for non-constant integers: arithmetic
/// over bare `int` givens would materialise Cartesian products of
/// `OXIDE_INT_MIN..OXIDE_INT_MAX` via `GroundDomain::apply_i32` during parse
/// (e.g. BIBD `letting b be (l*v*(v-1))/(k*(k-1))`).
fn value_letting_domain(expr: &Expression) -> Option<DomainPtr> {
    match expr.return_type() {
        ReturnType::Bool => Some(Domain::bool()),
        ReturnType::Int => {
            if let Some(Literal::Int(value)) = eval_constant(expr) {
                return Some(Domain::int(vec![Range::Single(value)]));
            }
            IntVal::new_expr(Moo::new(expr.clone()))
                .ok()
                .map(|value| Domain::int(vec![Range::Single(value)]))
        }
        _ => expr.domain_of(),
    }
}

/// Parse a letting statement into a SymbolTable containing the declared symbols
pub fn parse_letting_statement(
    ctx: &mut ParseContext,
    letting_statement: Node,
) -> Result<Option<SymbolTable>, FatalParseError> {
    let Some(keyword) = field!(recover, ctx, letting_statement, "letting_keyword") else {
        return Ok(None);
    };
    span_with_hover(
        &keyword,
        ctx.source_code,
        ctx.source_map,
        HoverInfo {
            description: "Letting keyword".to_string(),
            doc_key: None,
            kind: Some(SymbolKind::Letting),
            ty: None,
            decl_span: None,
        },
    );

    let mut symbol_table = SymbolTable::new();

    for variable_decl in named_children(&letting_statement) {
        let mut temp_symbols = BTreeSet::new();

        let Some(variable_list) = field!(recover, ctx, variable_decl, "variable_list") else {
            return Ok(None);
        };
        for variable in named_children(&variable_list) {
            let variable_name = &ctx.source_code[variable.start_byte()..variable.end_byte()];

            if is_keyword_identifier(variable_name) {
                ctx.errors.push(RecoverableParseError::new(
                    format!("Keyword '{variable_name}' used as identifier"),
                    Some(variable.range()),
                ));
                // still add variable to symbol table to avoid follow-up errors
            }

            // Check for duplicate within the same statement
            if temp_symbols.contains(variable_name) {
                ctx.errors.push(RecoverableParseError::new(
                    format!(
                        "Variable '{}' is already declared in this letting statement",
                        variable_name
                    ),
                    Some(variable.range()),
                ));
                // don't return here, as we can still add the other variables to the symbol table
                continue;
            }

            // Check for duplicate declaration across statements
            let name = Name::user(variable_name);
            if let Some(symbols) = &ctx.symbols
                && symbols.read().lookup(&name).is_some()
            {
                let previous_line = ctx.lookup_decl_line(&name);
                ctx.errors.push(RecoverableParseError::new(
                    match previous_line {
                        Some(line) => format!(
                            "Variable '{}' is already declared in a previous statement on line {}",
                            variable_name, line
                        ),
                        None => format!(
                            "Variable '{}' is already declared in a previous statement",
                            variable_name
                        ),
                    },
                    Some(variable.range()),
                ));
                // don't return here, as we can still add the other variables to the symbol table
                continue;
            }

            temp_symbols.insert(variable_name);
            let hover = HoverInfo {
                description: format!("Letting variable: {variable_name}"),
                doc_key: None,
                kind: Some(SymbolKind::LettingVar),
                ty: None,
                decl_span: None,
            };
            let span_id = span_with_hover(&variable, ctx.source_code, ctx.source_map, hover);
            ctx.save_decl_span(name, span_id);
        }

        let Some(expr_or_domain) = field!(recover, ctx, variable_decl, "expr_or_domain") else {
            return Ok(None);
        };

        if variable_decl.child_by_field_name("domain").is_some() {
            for name in temp_symbols {
                let Some(domain) = parse_domain(ctx, expr_or_domain)? else {
                    continue;
                };

                symbol_table.insert(DeclarationPtr::new_domain_letting(Name::user(name), domain));
            }
        } else {
            for name in temp_symbols {
                let Some(expr) = parse_expression(ctx, expr_or_domain)? else {
                    continue;
                };
                let declaration = match value_letting_domain(&expr) {
                    Some(domain) => DeclarationPtr::new_value_letting_with_domain(
                        Name::user(name),
                        expr,
                        domain,
                    ),
                    None => DeclarationPtr::new_value_letting(Name::user(name), expr),
                };
                symbol_table.insert(declaration);
            }
        }
    }

    Ok(Some(symbol_table))
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::parser::parse_model::parse_essence;
    use conjure_cp_core::ast::{DeclarationKind, Name};
    use std::ops::Deref;
    use std::time::{Duration, Instant};

    /// Arithmetic lettings over bare `int` givens must use a symbolic singleton domain and
    /// finish parsing quickly (not enumerate the full Oxide integer range).
    #[test]
    fn arithmetic_letting_over_bare_int_givens_uses_symbolic_domain() {
        let src = r#"
language ESSENCE' 1.0
given v, k, l : int
letting b be (l*v*(v-1))/(k*(k-1))
find x: bool
such that x
"#;
        let started = Instant::now();
        let (model, _) = parse_essence(src).expect("model should parse");
        assert!(
            started.elapsed() < Duration::from_secs(2),
            "parse hung for {:?}; eager domain_of over bare int is likely back",
            started.elapsed()
        );

        let symbols = model.symbols();
        let decl = symbols
            .lookup(&Name::user("b"))
            .expect("letting b should be in the symbol table");
        match decl.kind().deref() {
            DeclarationKind::ValueLetting(_, Some(domain)) => {
                let ranges = domain.as_int().expect("letting b domain should be int");
                assert_eq!(
                    ranges.len(),
                    1,
                    "expected a singleton domain, got {ranges:?}"
                );
                assert!(
                    matches!(&ranges[0], Range::Single(IntVal::Expr(_))),
                    "expected symbolic IntVal::Expr singleton, got {:?}",
                    ranges[0]
                );
            }
            other => panic!("expected ValueLetting with retained domain, got {other:?}"),
        }
    }

    /// Constant integer lettings must keep a ground singleton so dependent domains stay tight.
    #[test]
    fn constant_int_letting_keeps_ground_singleton_domain() {
        let src = r#"
language ESSENCE' 1.0
letting n be 3
find x: int(1..10**n)
such that x = 1
"#;
        let (model, _) = parse_essence(src).expect("model should parse");
        let symbols = model.symbols();
        let decl = symbols
            .lookup(&Name::user("n"))
            .expect("letting n should be in the symbol table");
        match decl.kind().deref() {
            DeclarationKind::ValueLetting(_, Some(domain)) => {
                let ranges = domain.as_int().expect("letting n domain should be int");
                assert_eq!(
                    ranges.len(),
                    1,
                    "expected a singleton domain, got {ranges:?}"
                );
                assert!(
                    matches!(&ranges[0], Range::Single(IntVal::Const(3))),
                    "expected ground IntVal::Const(3) singleton, got {:?}",
                    ranges[0]
                );
            }
            other => panic!("expected ValueLetting with retained domain, got {other:?}"),
        }
    }
}
