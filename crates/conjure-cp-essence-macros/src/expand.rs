use conjure_cp_essence_parser::{
    EssenceParseError,
    expression::parse_expression,
    util::{get_tree, query_toplevel},
};
use polyquine::Quine;
use proc_macro2::{TokenStream, TokenTree};
use quote::{ToTokens, quote};
use syn::{Error, LitStr, Result};
use tree_sitter::Node;

pub fn expand_expr(essence: &TokenTree) -> Result<TokenStream> {
    let src = to_src(essence);
    let (tree, source_code) =
        get_tree(&src).ok_or(Error::new(essence.span(), "Could not parse Essence AST"))?;
    let root = tree.root_node();

    // Get top level expressions
    let mut query = query_toplevel(&root, &|n| n.kind() == "expression");
    let expr_node = query
        .next()
        .ok_or(Error::new(essence.span(), "Expected an Essence expression"))?;

    // We only expect one expression, error if that's not the case
    if let Some(expr) = query.next() {
        let tokens = &source_code[expr.start_byte()..expr.end_byte()];
        return Err(Error::new(
            essence.span(),
            format!(
                "Unexpected tokens: `{tokens}`. Expected a single Essence expression. Perhaps you meant `essence_vec!`?"
            ),
        ));
    }

    // Parse expression and build the token stream
    let expr = mk_expr(expr_node, &source_code, &root, essence)?;
    Ok(expr)
}

pub fn expand_expr_vec(tt: &TokenTree) -> Result<TokenStream> {
    let mut ans: Vec<TokenStream> = Vec::new();
    let src = to_src(tt);
    let (tree, source_code) =
        get_tree(&src).ok_or(Error::new(tt.span(), "Could not parse Essence AST"))?;
    let root = tree.root_node();

    let query = query_toplevel(&root, &|n| n.kind() == "expression");
    for expr_node in query {
        let expr = mk_expr(expr_node, &source_code, &root, tt)?;
        ans.push(expr);
    }
    Ok(quote! { vec![#(#ans),*] })
}

/// Parse a single expression or make a compile time error
fn mk_expr(node: Node, src: &str, root: &Node, tt: &TokenTree) -> Result<TokenStream> {
    match parse_expression(node, src, root, None) {
        Ok(expr) => Ok(expr.ctor_tokens()),
        Err(err) => {
            let error_message = match err {
                EssenceParseError::SyntaxError {
                    msg,
                    range: Some(rng),
                } => {
                    let lines: Vec<&str> = src.lines().collect();
                    let start_line = rng.start_point.row;
                    let mut start_col = rng.start_point.column;

                    let mut line_content = lines
                        .get(start_line)
                        .unwrap_or(&"<line not found>")
                        .trim()
                        .to_string();
                    if line_content.starts_with("such that") {
                        let len = "such that".len();
                        line_content = line_content[len..].trim_start().to_string();
                        start_col -= len;
                    }

                    format!(
                        "Syntax error: {}\n{}\n{}^-- Error here",
                        msg,
                        line_content,
                        " ".repeat(start_col)
                    )
                }
                _ => err.to_string(),
            };

            Err(Error::new(tt.span(), error_message))
        }
    }
}

/// Parse string literals (gets rid of ""), otherwise use tokens as is
fn to_src(tt: &TokenTree) -> String {
    match syn::parse::<LitStr>(tt.to_token_stream().into()) {
        Ok(src) => src.value(),
        Err(_) => tt.to_string(),
    }
}
