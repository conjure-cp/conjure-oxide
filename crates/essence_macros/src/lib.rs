use conjure_essence_parser::expression::parse_expression;
use conjure_essence_parser::util::{get_metavars, get_tree, query_toplevel};
use proc_macro::TokenStream;
#[allow(unused)]
use uniplate::Uniplate;

use quote::quote;
use syn::{
    parse::Parse, parse::ParseStream, parse_macro_input,
    LitStr, Result,
};

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
pub fn essence_expr(args: TokenStream) -> TokenStream {
    let EssenceExprArgs { essence } = parse_macro_input!(args as EssenceExprArgs);
    let (tree, src) = get_tree(&essence.value()).unwrap();
    let root = tree.root_node();
    let metavars = get_metavars(&root, &src);
    let mut exprs = query_toplevel(&root, &|n| n.kind() == "expression")
        .map(|node| parse_expression(node, &src, &root));
    // TODO: We know how to get the expressions, but ideally we want the macro to use this information to construct the AST at compile time
    // and output a quote! with the correct manual instantiation of Expression variants, instead of just generating a call to `conjure_essence_parser` functions at runtime.
    // This is definitely possible but some work is needed to implement it.
    // A rough sketch would be:
    // 1. Traverse the parsed AST in DFS order
    // 2. If we see a Metavar node, replace it with the appropriate `Ident` (if it's not in scope or not an `Expression`, the macro will cause a compile error which is what we want)
    // 3. Otherwise, codegen the right `Expression::...` variant with arguments (which we have just parsed so we know it, just need to get it out via `quote` somehow)
    // 4. When the process is over, we should have a well-formed manually defined AST with no extra runtime overhead / dependencies.
    let expr = exprs
        .next()
        .unwrap_or_else(|| panic!("No expressions found"));

    quote! {
        #expr
    }
    .into()
}

mod test {
    
    

    #[test]
    pub fn test_to_tokens() {
        let expr = parse_expr("x + 2").unwrap();
        println!("{}", quote! {#expr})
    }
}
