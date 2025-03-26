pub use quote::{quote, ToTokens};
use uniplate::{Biplate, Uniplate};

use super::AbstractLiteral;

fn vec_to_tokens<T: ToTokens>(vec: &Vec<T>) -> proc_macro2::TokenStream {
    quote! { vec![#(#vec),*] }
}

impl<T> ToTokens for AbstractLiteral<T>
where
    T: Uniplate + Biplate<AbstractLiteral<T>> + Biplate<T> + ToTokens,
{
    fn to_tokens(&self, tokens: &mut proc_macro2::TokenStream) {
        match self {
            AbstractLiteral::Set(items) => {
                let item_toks = vec_to_tokens(items);
                tokens.extend(quote! {
                    conjure_core::ast::AbstractLiteral::Set(#item_toks)
                });
            }
            AbstractLiteral::Matrix(items, domain) => {
                let item_toks = vec_to_tokens(items);
                tokens.extend(quote! {
                    conjure_core::ast::AbstractLiteral::Matrix(#item_toks, #domain)
                });
            }
        }
    }
}
