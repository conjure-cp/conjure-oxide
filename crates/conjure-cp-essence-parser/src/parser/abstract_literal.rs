use crate::expression::parse_expression;
use crate::parser::domain::parse_domain;
use crate::util::named_children;
use crate::{EssenceParseError, field};
use conjure_cp_core::ast::{AbstractLiteral, Domain, Expression, SymbolTable};
use conjure_cp_core::{domain_int, range};
use tree_sitter::Node;

pub fn parse_abstract(
    node: &Node,
    source_code: &str,
    symbols: Option<&SymbolTable>,
) -> Result<AbstractLiteral<Expression>, EssenceParseError> {
    match node.kind() {
        "record" => parse_record(node, source_code, symbols),
        "tuple" => parse_tuple(node, source_code, symbols),
        "matrix" => parse_matrix(node, source_code, symbols),
        _ => Err(EssenceParseError::syntax_error(
            format!("Expected abstract literal, got: '{}'", node.kind()),
            Some(node.range()),
        )),
    }
}

fn parse_record(
    node: &Node,
    source_code: &str,
    symbols: Option<&SymbolTable>,
) -> Result<AbstractLiteral<Expression>, EssenceParseError> {
    let mut values = Vec::new();
    for child in node.children_by_field_name("name_value_pair", &mut node.walk()) {
        let name_node = field!(child, "name");
        let name_str = &source_code[name_node.start_byte()..name_node.end_byte()];
        let name = conjure_cp_core::ast::Name::user(name_str);

        let value: Expression =
            parse_expression(field!(child, "value"), source_code, node, symbols)?;
        values.push(conjure_cp_core::ast::records::RecordValue { name, value });
    }
    Ok(AbstractLiteral::Record(values))
}

fn parse_tuple(
    node: &Node,
    source_code: &str,
    symbols: Option<&SymbolTable>,
) -> Result<AbstractLiteral<Expression>, EssenceParseError> {
    let mut elements = Vec::new();
    for child in named_children(node) {
        elements.push(parse_expression(child, source_code, node, symbols)?);
    }
    Ok(AbstractLiteral::Tuple(elements))
}

fn parse_matrix(
    node: &Node,
    source_code: &str,
    symbols: Option<&SymbolTable>,
) -> Result<AbstractLiteral<Expression>, EssenceParseError> {
    let mut elements = vec![];
    let mut domain: Option<Domain> = None;
    for child in named_children(node) {
        if child.kind() == "arithmetic_expr" {
            elements.push(parse_expression(child, source_code, node, symbols)?);
        } else {
            domain = Some(parse_domain(child, source_code)?);
        }
    }
    if domain.is_none() {
        let count = elements.len() as i32;
        domain = Some(domain_int!(1..count));
    }

    Ok(AbstractLiteral::Matrix(elements, Box::new(domain.unwrap())))
}
