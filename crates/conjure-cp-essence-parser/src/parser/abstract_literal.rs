use crate::expression::parse_expression;
use crate::util::named_children;
use crate::{EssenceParseError, field};
use conjure_cp_core::ast::{AbstractLiteral, Expression, SymbolTable};
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
    let elements = parse_child_exprs(node, source_code, symbols)?;
    Ok(AbstractLiteral::Tuple(elements))
}

fn parse_matrix(
    node: &Node,
    source_code: &str,
    symbols: Option<&SymbolTable>,
) -> Result<AbstractLiteral<Expression>, EssenceParseError> {
    let elements = parse_child_exprs(&field!(node, "elements"), source_code, symbols)?;
    let sz = elements.len() as i32;
    Ok(AbstractLiteral::Matrix(
        elements,
        Box::new(domain_int!(1..sz)),
    ))
}

fn parse_child_exprs(
    node: &Node,
    source_code: &str,
    symbols: Option<&SymbolTable>,
) -> Result<Vec<Expression>, EssenceParseError> {
    let mut exprs = Vec::new();
    for child in named_children(node) {
        exprs.push(parse_expression(child, source_code, node, symbols)?);
    }
    Ok(exprs)
}
