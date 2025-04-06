use super::expression::parse_expr_to_ts;
use conjure_essence_parser::util::{get_tree, query_toplevel};
use proc_macro2::{TokenStream, TokenTree};
use quote::quote;
use syn::{Error, Result};

pub fn expand_expr(essence: &TokenTree) -> Result<TokenStream> {
    let src = essence.to_string();
    let (tree, source_code) =
        get_tree(&src).ok_or(Error::new(essence.span(), "Could not parse Essence AST"))?;
    let root = tree.root_node();

    let mut query = query_toplevel(&root, &|n| n.kind() == "expression");
    let expr_node = query
        .next()
        .ok_or(Error::new(essence.span(), "Expected an Essence expression"))?;
    if let Some(expr) = query.next() {
        let tokens = &source_code[expr.start_byte()..expr.end_byte()];
        return Err(Error::new(essence.span(), format!("Unexpected tokens: `{}`. Expected a single Essence expression. Perhaps you meant `essence_exprs!`?", tokens)));
    }

    let expr = match parse_expr_to_ts(expr_node, &source_code, &root) {
        Ok(expr) => Ok(expr),
        Err(err) => {
            let msg = format!("Error parsing Essence expression: {}", err);
            Err(Error::new(essence.span(), msg))
        }
    }?;

    Ok(expr)
}

pub fn expand_expr_vec(exprs: TokenStream) -> Result<TokenStream> {
    let mut ans: Vec<TokenStream> = Vec::new();
    for tt in exprs.into_iter() {
        let src = tt.to_string();
        let (tree, source_code) =
            get_tree(&src).ok_or(Error::new(tt.span(), "Could not parse Essence AST"))?;
        let root = tree.root_node();

        let query = query_toplevel(&root, &|n| n.kind() == "expression");
        for expr_node in query {
            let expr = match parse_expr_to_ts(expr_node, &source_code, &root) {
                Ok(expr) => Ok(expr),
                Err(err) => {
                    let msg = format!("Error parsing Essence expression: {}", err);
                    Err(Error::new(tt.span(), msg))
                }
            }?;
            ans.push(expr);
        }
    }
    Ok(quote! { vec![#(#ans),*] })
}
