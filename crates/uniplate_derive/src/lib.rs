mod ast;
mod prelude;
mod state;

use prelude::*;
use syn::parse_macro_input;

#[proc_macro_derive(Uniplate)]
pub fn uniplate_derive(input: TokenStream) -> TokenStream {
    let input = parse_macro_input!(input as ast::Data);
    //let mut state: ParserState = ParserState::new(input);
    //eprintln!("HERE");

    // TODO: populate state with types to generate biplates to.

    let mut out_tokens: Vec<TokenStream2> = Vec::new();

    //// Generate all the biplates
    //while let Some(to) = state.tos_left.pop_front() {
    //    state.to = Some(to);
    //    out_tokens.push(derive_a_biplate(&mut state));
    //}

    //out_tokens.into_iter().collect::<TokenStream2>().into()
    syn::Error::new(input.span(), format!("{:#?}", input))
        .to_compile_error()
        .into()
}

fn derive_a_biplate(_state: &mut ParserState) -> TokenStream2 {
    todo!()
}
