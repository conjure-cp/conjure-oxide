use conjure_essence_parser::expression::parse_expression;
use conjure_essence_parser::util::{get_metavars, get_tree, query_toplevel};
use proc_macro::TokenStream;
#[allow(unused)]
use uniplate::Uniplate;

use quote::quote;
use syn::{parse::Parse, parse::ParseStream, parse_macro_input, LitStr, Result};

mod expression;

struct EssenceExprArgs {
    essence: LitStr,
}

impl Parse for EssenceExprArgs {
    fn parse(input: ParseStream) -> Result<Self> {
        let essence = input.parse::<LitStr>()?;
        Ok(Self { essence })
    }
}

#[proc_macro]
pub fn essence_expr(args: TokenStream) -> TokenStream {}
