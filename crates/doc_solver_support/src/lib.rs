use std::collections::HashMap;

use proc_macro::TokenStream;
use quote::quote;
use syn::{parse_macro_input, visit_mut::VisitMut, ItemEnum, Variant};

// A nice S.O answer that helped write the syn code :)
// https://stackoverflow.com/a/65182902

struct RemoveSolverAttrs;
impl VisitMut for RemoveSolverAttrs {
    fn visit_variant_mut(&mut self, i: &mut Variant) {
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
/// ```rust
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

            attr.parse_nested_meta(|meta| {
                let ident = meta.path.require_ident()?;
                let solver_str: String = ident.to_string().to_lowercase();
                match nodes_supported_by_solver.get_mut(&solver_str) {
                    None => {
                        nodes_supported_by_solver.insert(solver_str, vec![variant_ident.clone()]);
                        ()
                    }
                    Some(a) => {
                        a.push(variant_ident.clone());
                        ()
                    }
                };
                return Ok(());
            })
            .unwrap();
        }
    }

    // we must remove all references to #[solver] before we finish expanding the macro,
    // as it does not exist outside of the context of this macro.
    RemoveSolverAttrs.visit_item_enum_mut(&mut input);

    // Build the doc string.

    // Note that quote wants us to build the doc message first, as it cannot interpolate doc
    // comments well.
    // https://docs.rs/quote/latest/quote/macro.quote.html#interpolating-text-inside-of-doc-comments
    let mut doc_msg: String = "# Supported AST Nodes for Solvers\n".into();
    for solver in nodes_supported_by_solver.keys() {
        // a nice title
        doc_msg.push_str(&format!("## `{}`\n```rust\n", solver));

        // list all the ast nodes for this solver
        for node in nodes_supported_by_solver.get(solver).unwrap() {
            doc_msg.push_str(&format!("{}\n", node.to_string()));
        }

        // end the code block
        doc_msg.push_str(&format!("```\n"));
    }

    let expanded = quote! {
        #[doc = #doc_msg]
        #input
    };

    TokenStream::from(expanded)
}
