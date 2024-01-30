use std::collections::HashMap;

use itertools::Itertools;
use proc_macro::TokenStream;
use quote::quote;
use syn::{
    parse_macro_input, parse_quote, punctuated::Punctuated, visit_mut::VisitMut, Attribute,
    ItemEnum, Meta, Token, Variant,
};

// A nice S.O answer that helped write the syn code :)
// https://stackoverflow.com/a/65182902

struct RemoveSolverAttrs;
impl VisitMut for RemoveSolverAttrs {
    fn visit_variant_mut(&mut self, i: &mut Variant) {
        // 1. generate docstring for variant
        // Supported by: minion, sat ...
        //
        // 2. delete #[solver] attributes

        let mut solvers: Vec<String> = vec![];
        for attr in i.attrs.iter() {
            if !attr.path().is_ident("solver") {
                continue;
            }
            let nested = attr
                .parse_args_with(Punctuated::<Meta, Token![,]>::parse_terminated)
                .unwrap();
            for arg in nested {
                let ident = arg.path().require_ident().unwrap();
                let solver_name = ident.to_string();
                solvers.push(solver_name);
            }
        }

        if !solvers.is_empty() {
            let solver_list: String = solvers.into_iter().intersperse(", ".into()).collect();
            let doc_string: String = format!("**Supported by:** {}.\n", solver_list);
            let doc_attr: Attribute = parse_quote!(#[doc = #doc_string]);
            i.attrs.push(doc_attr);
        }

        i.attrs = i
            .attrs
            .iter()
            .filter(|attr| !attr.path().is_ident("solver"))
            .map(|attr| attr.clone())
            .collect();

        return;
    }
}

/// A macro to document the AST's variants by the solvers they support.
///
/// The Conjure-Oxide AST is used as the singular intermediate language between input and solvers.
/// A consequence of this is that the AST contains all possible supported expressions for all
/// solvers, as well as the high level Essence language we take as input. A given
/// solver only "supports" a small subset of the AST, and will reject the rest.
///
/// The documentation this generates helps determine which AST nodes are used for which backends,
/// to help rule writers.
///
/// # Example
///
/// ```
/// use doc_solver_support::doc_solver_support;
///
/// #[doc_solver_support]
/// pub enum Expression {
///    #[solver(Minion)]
///    ConstantInt(i32),
///    // ...
///    #[solver(Chuffed)]
///    #[solver(Minion)]
///    Sum(Vec<Expression>)
///    }
/// ```
///
/// The Expression type will have the following lists appended to its documentation:
///
///```text
/// ## Supported by `minion`
///    ConstantInt(i32)
///    Sum(Vec<Expression>)
///
///
/// ## Supported by `chuffed`
///    ConstantInt(i32)
///    Sum(Vec<Expression>)
/// ```
///
/// Two equivalent syntaxes exist for specifying supported solvers:
///
/// ```
///# use doc_solver_support::doc_solver_support;
///#
///# #[doc_solver_support]
///# pub enum Expression {
///#    #[solver(Minion)]
///#    ConstantInt(i32),
///#    // ...
///     #[solver(Chuffed)]
///     #[solver(Minion)]
///     Sum(Vec<Expression>)
///#    }
/// ```
///
/// ```
///# use doc_solver_support::doc_solver_support;
///#
///# #[doc_solver_support]
///# pub enum Expression {
///#    #[solver(Minion)]
///#    ConstantInt(i32),
///#    // ...
///     #[solver(Minion,Chuffed)]
///     Sum(Vec<Expression>)
///#    }
/// ```
///
#[proc_macro_attribute]
pub fn doc_solver_support(_attr: TokenStream, input: TokenStream) -> TokenStream {
    // Parse the input tokens into a syntax tree
    let mut input = parse_macro_input!(input as ItemEnum);
    let mut nodes_supported_by_solver: HashMap<String, Vec<syn::Ident>> = HashMap::new();

    // process each item inside the enum.
    for variant in input.variants.iter() {
        let variant_ident = variant.ident.clone();
        for attr in variant.attrs.iter() {
            if !attr.path().is_ident("solver") {
                continue;
            }

            let nested = attr
                .parse_args_with(Punctuated::<Meta, Token![,]>::parse_terminated)
                .unwrap();
            for arg in nested {
                let ident = arg.path().require_ident().unwrap();
                let solver_name = ident.to_string();
                match nodes_supported_by_solver.get_mut(&solver_name) {
                    None => {
                        nodes_supported_by_solver.insert(solver_name, vec![variant_ident.clone()]);
                        ()
                    }
                    Some(a) => {
                        a.push(variant_ident.clone());
                        ()
                    }
                };
            }
        }
    }

    // we must remove all references to #[solver] before we finish expanding the macro,
    // as it does not exist outside of the context of this macro.
    RemoveSolverAttrs.visit_item_enum_mut(&mut input);

    // Build the doc string.

    // Note that quote wants us to build the doc message first, as it cannot interpolate doc
    // comments well.
    // https://docs.rs/quote/latest/quote/macro.quote.html#interpolating-text-inside-of-doc-comments
    let mut doc_msg: String = "# Solver Support\n".into();
    for solver in nodes_supported_by_solver.keys() {
        // a nice title
        doc_msg.push_str(&format!("## {}\n", solver));

        // list all the ast nodes for this solver
        for node in nodes_supported_by_solver
            .get(solver)
            .unwrap()
            .iter()
            .map(|x| x.to_string())
            .sorted()
        {
            doc_msg.push_str(&format!("* [`{}`]({}::{})\n", node, input.ident, node));
        }

        // end list
        doc_msg.push_str("\n");
    }

    input.attrs.push(parse_quote!(#[doc = #doc_msg]));
    let expanded = quote! {
        #input
    };

    TokenStream::from(expanded)
}
