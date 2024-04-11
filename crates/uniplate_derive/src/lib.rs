mod ast;
mod prelude;
mod state;

use prelude::*;
use syn::parse_macro_input;

#[proc_macro_derive(Uniplate)]
pub fn uniplate_derive(input: TokenStream) -> TokenStream {
    let input = parse_macro_input!(input as ast::Data);
    let mut state: ParserState = ParserState::new(input.clone());

    let mut out_tokens: Vec<TokenStream2> = Vec::new();

    //// Generate all the biplates
    while let Some(to) = state.tos_left.pop_front() {
        state.to = Some(to);
        out_tokens.push(derive_a_biplate(&mut state));
        out_tokens.push(derive_a_uniplate(&mut state));
    }

    //syn::Error::new(input.span(), format!("{:#?}", input))
    //    .to_compile_error()
    //    .into()
    out_tokens.into_iter().collect::<TokenStream2>().into()
}

fn derive_a_uniplate(state: &mut ParserState) -> TokenStream2 {
    let from = state.to.to_token_stream();
    quote! {
        impl ::uniplate::biplate::Uniplate for #from {
            fn uniplate(&self) -> (::uniplate::Tree<#from>, Box<dyn Fn(::uniplate::Tree<#from>) -> #from>) {
                todo!()
            }
        }
    }
}
fn derive_a_biplate(state: &mut ParserState) -> TokenStream2 {
    let from = state.from.base_typ.to_token_stream();
    let to = state.to.to_token_stream();

    quote! {
        impl ::uniplate::biplate::Biplate<#to> for #from {
            fn biplate(&self) -> (::uniplate::Tree<#to>, Box<dyn Fn(::uniplate::Tree<#to>) -> #to>) {
                todo!()
            }
        }
    }
}
