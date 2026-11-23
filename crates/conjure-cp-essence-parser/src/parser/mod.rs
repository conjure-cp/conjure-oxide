mod atom;
mod comprehension;
pub mod domain;
mod dominance;
pub mod expression;
mod macros;
mod objective;
pub mod parse_exprs;
pub mod parse_literal;
pub mod parse_model;
pub mod statement;
pub mod syntax_errors;
pub mod traversal;
pub mod util;

use conjure_cp_core::ast::Name;
pub use parse_exprs::*;
pub use parse_literal::*;
pub use parse_model::*;
pub use util::ParseContext;

use tree_sitter::Node;

use crate::FatalParseError;

fn parse_ident(ctx: &mut ParseContext, node: &Node<'_>) -> String {
    ctx.source_code[node.start_byte()..node.end_byte()].into()
}

fn parse_name(ctx: &mut ParseContext, node: &Node) -> Result<Name, FatalParseError> {
    Ok(Name::User(parse_ident(ctx, &node).into()))
}
