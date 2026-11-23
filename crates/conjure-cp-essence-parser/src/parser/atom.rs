use crate::errors::FatalParseError;
use crate::named_child;
use crate::parser::ParseContext;

use conjure_cp_core::ast::Literal;

use conjure_cp_core::error::Error;
use tree_sitter::Node;

pub fn parse_constant(ctx: &mut ParseContext, node: &Node) -> Result<Literal, FatalParseError> {
    let inner = named_child!(node);
    let lit = match inner.kind() {
        "integer" => Literal::Int(parse_int(ctx, &inner)?),
        "true" => Literal::Bool(true),
        "false" => Literal::Bool(false),
        _ => unreachable!(),
    };
    Ok(lit)
}

pub(crate) fn parse_int(ctx: &mut ParseContext, node: &Node) -> Result<i32, FatalParseError> {
    dbg!(ctx.source_code, node.start_byte(), node.end_byte());
    let raw_value = &ctx.source_code[node.start_byte()..node.end_byte()];
    raw_value
        .parse::<i32>()
        .map_err(|_| FatalParseError::Other(Error::Parse(raw_value.to_owned())))
}

// pub fn parse_variable(
//     ctx: &mut ParseContext,
//     node: &Node,
// ) -> Result<Option<Atom>, FatalParseError> {
//     let raw_name = &ctx.source_code[node.start_byte()..node.end_byte()];
//
//     let name = Name::user(raw_name.trim());
//     if let Some(symbols) = &ctx.symbols {
//         let lookup_result = {
//             let symbols_read = symbols.read();
//             symbols_read.lookup(&name)
//         };
//
//         if let Some(decl) = lookup_result {
//             let symbol_kind = match &decl.kind().clone() as &DeclarationKind {
//                 DeclarationKind::Find(_) | DeclarationKind::FindAuxiliary(_) => SymbolKind::FindVar,
//                 DeclarationKind::Given(_) => SymbolKind::GivenVar,
//                 DeclarationKind::ValueLetting(_, _) => SymbolKind::LettingVar,
//                 DeclarationKind::TemporaryValueLetting(_) => SymbolKind::LettingVar,
//                 DeclarationKind::DomainLetting(_) => SymbolKind::LettingVar,
//                 DeclarationKind::Quantified(..) => SymbolKind::FindVar,
//                 DeclarationKind::QuantifiedExpr(..) => SymbolKind::FindVar,
//                 &_ => todo!(),
//             };
//
//             let hover = HoverInfo {
//                 description: format!("Variable: {name}"),
//                 doc_key: None,
//                 kind: Some(symbol_kind),
//                 ty: decl.domain().map(|d| d.to_string()),
//                 decl_span: ctx.lookup_decl_span(&name),
//             };
//             span_with_hover(node, ctx.source_code, ctx.source_map, hover);
//
//             Ok(Some(Atom::Reference(conjure_cp_core::ast::Reference::new(
//                 decl,
//             ))))
//         } else {
//             ctx.record_error(RecoverableParseError::new(
//                 format!("The identifier '{}' is not defined", raw_name),
//                 Some(node.range()),
//             ));
//             Ok(None)
//         }
//     } else {
//         ctx.record_error(RecoverableParseError::new(
//             format!("Symbol table missing when parsing variable '{raw_name}'"),
//             Some(node.range()),
//         ));
//         Ok(None)
//     }
// }
