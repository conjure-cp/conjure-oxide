//! A macro to document enum variants with the things that they are compatible with.
//!
//!
//! As well as documenting each variant, this macro also generates lists of all compatible variants
//! for each "thing".
//!
//! # Motivation
//!
//! This macro is used in Conjure-Oxide, a constraint modelling tool with support for multiple
//! backend solvers (e.g. Minion, SAT).
//!
//! The Conjure-Oxide AST is used as the singular representation for constraints models throughout
//! its crate. A consequence of this is that the AST must contain all possible supported
//! expressions for all solvers, as well as the high level Essence language it takes as input.
//! Therefore, only a small subset of AST nodes are useful for a particular solver.
//!
//! The documentation this generates helps rewrite rule implementers determine which AST nodes are
//! used for which backends by grouping AST nodes per solver.

#![allow(clippy::unwrap_used)]
#![allow(unstable_name_collisions)]

use proc_macro::TokenStream;
use std::collections::HashMap;

use itertools::Itertools;
use quote::quote;
use syn::{
    Attribute, ItemEnum, Meta, Token, Variant, parse_macro_input, parse_quote,
    punctuated::Punctuated, visit_mut::VisitMut,
};

// A nice S.O answer that helped write the syn code :)
// https://stackoverflow.com/a/65182902

struct RemoveCompatibleAttrs;
impl VisitMut for RemoveCompatibleAttrs {
    fn visit_variant_mut(&mut self, i: &mut Variant) {
        // 1. generate docstring for variant
        // Supported by: minion, sat ...
        //
        // 2. delete #[compatible] attributes

        let mut solvers: Vec<String> = vec![];
        for attr in i.attrs.iter() {
            if !attr.path().is_ident("compatible") {
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
            let doc_string: String = format!("**Supported by:** {solver_list}.\n");
            let doc_attr: Attribute = parse_quote!(#[doc = #doc_string]);
            i.attrs.push(doc_attr);
        }

        i.attrs.retain(|attr| !attr.path().is_ident("compatible"));
    }
}

/// A macro to document enum variants by the things that they are compatible with.
///
/// # Examples
///
/// ```
/// use conjure_cp_enum_compatibility_macro::document_compatibility;
///
/// #[document_compatibility]
/// pub enum Expression {
///    #[compatible(Minion)]
///    ConstantInt(i32),
///    // ...
///    #[compatible(Chuffed)]
///    #[compatible(Minion)]
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
///# use conjure_cp_enum_compatibility_macro::document_compatibility;
///#
///# #[document_compatibility]
///# pub enum Expression {
///#    #[compatible(Minion)]
///#    ConstantInt(i32),
///#    // ...
///     #[compatible(Chuffed)]
///     #[compatible(Minion)]
///     Sum(Vec<Expression>)
///#    }
/// ```
///
/// ```
///# use conjure_cp_enum_compatibility_macro::document_compatibility;
///#
///# #[document_compatibility]
///# pub enum Expression {
///#    #[compatible(Minion)]
///#    ConstantInt(i32),
///#    // ...
///     #[compatible(Minion,Chuffed)]
///     Sum(Vec<Expression>)
///#    }
/// ```
///
#[proc_macro_attribute]
pub fn document_compatibility(_attr: TokenStream, input: TokenStream) -> TokenStream {
    // Parse the input tokens into a syntax tree
    let mut input = parse_macro_input!(input as ItemEnum);
    let mut nodes_supported_by_solver: HashMap<String, Vec<syn::Ident>> = HashMap::new();

    // process each item inside the enum.
    for variant in input.variants.iter() {
        let variant_ident = variant.ident.clone();
        for attr in variant.attrs.iter() {
            if !attr.path().is_ident("compatible") {
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
                    }
                    Some(a) => {
                        a.push(variant_ident.clone());
                    }
                };
            }
        }
    }

    // we must remove all references to #[compatible] before we finish expanding the macro,
    // as it does not exist outside of the context of this macro.
    RemoveCompatibleAttrs.visit_item_enum_mut(&mut input);

    // Build the doc string.

    // Note that quote wants us to build the doc message first, as it cannot interpolate doc
    // comments well.
    // https://docs.rs/quote/latest/quote/macro.quote.html#interpolating-text-inside-of-doc-comments
    let mut doc_msg: String = "# Compatability\n".into();
    for solver in nodes_supported_by_solver.keys() {
        // a nice title
        doc_msg.push_str(&format!("## {solver}\n"));

        // list all the ast nodes for this solver
        for node in nodes_supported_by_solver
            .get(solver)
            .unwrap()
            .iter()
            .map(|x| x.to_string())
            .sorted()
        {
            doc_msg.push_str(&format!("* [`{node}`]({}::{node})\n", input.ident));
        }

        // end list
        doc_msg.push('\n');
    }

    input.attrs.push(parse_quote!(#[doc = #doc_msg]));
    let expanded = quote! {
        #input
    };

    TokenStream::from(expanded)
}
