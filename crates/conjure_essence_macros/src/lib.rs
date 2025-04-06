use proc_macro::TokenStream;
use proc_macro2::{Delimiter, Group, TokenStream as TokenStream2, TokenTree};

mod expand;
mod expression;

use expand::*;

#[proc_macro]
pub fn essence_expr(args: TokenStream) -> TokenStream {
    let ts = TokenStream2::from(args);
    let tt = TokenTree::Group(Group::new(Delimiter::None, ts));
    match expand_expr(&tt) {
        Ok(tokens) => tokens.into(),
        Err(err) => err.to_compile_error().into(),
    }
}

#[proc_macro]
pub fn essence_vec(args: TokenStream) -> TokenStream {
    let ts = TokenStream2::from(args);
    match expand_expr_vec(ts) {
        Ok(tokens) => tokens.into(),
        Err(err) => err.to_compile_error().into(),
    }
}
