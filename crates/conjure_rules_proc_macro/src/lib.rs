//! This is the backend procedural macro crate for `conjure_rules`. USE THAT INSTEAD!

use proc_macro::TokenStream;
use quote::quote;
use syn::punctuated::Punctuated;
use syn::token::Comma;
use syn::{
    parenthesized, parse::Parse, parse::ParseStream, parse_macro_input, Ident, ItemFn, LitInt,
    LitStr, Result, Token,
};

#[derive(Debug)]
struct RuleSetAndPriority {
    rule_set: LitStr,
    priority: LitInt,
}

impl Parse for RuleSetAndPriority {
    fn parse(input: ParseStream) -> Result<Self> {
        let content;
        parenthesized!(content in input);
        let rule_set: LitStr = content.parse()?;
        let _: Comma = content.parse()?;
        let priority: LitInt = content.parse()?;
        Ok(RuleSetAndPriority { rule_set, priority })
    }
}

#[derive(Debug)]
struct RegisterRuleArgs {
    pub rule_sets: Vec<RuleSetAndPriority>,
}

impl Parse for RegisterRuleArgs {
    fn parse(input: ParseStream) -> Result<Self> {
        let rule_sets = Punctuated::<RuleSetAndPriority, Comma>::parse_terminated(input)?;
        Ok(RegisterRuleArgs {
            rule_sets: rule_sets.into_iter().collect(),
        })
    }
}

#[proc_macro_attribute]
pub fn register_rule(arg_tokens: TokenStream, item: TokenStream) -> TokenStream {
    let func = parse_macro_input!(item as ItemFn);
    let rule_ident = &func.sig.ident;
    let static_name = format!("CONJURE_GEN_RULE_{}", rule_ident).to_uppercase();
    let static_ident = Ident::new(&static_name, rule_ident.span());

    let args = parse_macro_input!(arg_tokens as RegisterRuleArgs);
    let rule_sets = args
        .rule_sets
        .iter()
        .map(|rule_set| {
            let rule_set_name = &rule_set.rule_set;
            let priority = &rule_set.priority;
            quote! {
                (#rule_set_name, #priority as u8)
            }
        })
        .collect::<Vec<_>>();

    let expanded = quote! {
        #func

        #[::conjure_rules::_dependencies::distributed_slice(::conjure_rules::RULES_DISTRIBUTED_SLICE)]
        pub static #static_ident: ::conjure_rules::_dependencies::Rule = ::conjure_rules::_dependencies::Rule {
            name: stringify!(#rule_ident),
            application: #rule_ident,
            rule_sets: &[#(#rule_sets),*],
        };
    };

    TokenStream::from(expanded)
}

#[derive(Debug)]
struct RuleSetArgs {
    name: LitStr,
    dependencies: Vec<LitStr>,
}

impl Parse for RuleSetArgs {
    fn parse(input: ParseStream) -> Result<Self> {
        let name = input.parse()?;
        input.parse::<Token![,]>()?;

        let content;
        parenthesized!(content in input);

        let mut dependencies = Vec::new();

        while !content.is_empty() {
            let dep = content.parse()?;
            dependencies.push(dep);
            if content.is_empty() {
                break;
            }
            content.parse::<Comma>()?;
        }

        Ok(Self { name, dependencies })
    }
}

#[proc_macro]
pub fn register_rule_set(args: TokenStream) -> TokenStream {
    let RuleSetArgs { name, dependencies } = parse_macro_input!(args as RuleSetArgs);

    let dependencies = dependencies
        .into_iter()
        .map(|dep| quote! { #dep })
        .collect::<Vec<_>>();

    let expanded = quote! {
        #[::conjure_rule_sets::_dependencies::distributed_slice(::conjure_rule_sets::RULE_SETS_DISTRIBUTED_SLICE)]
        pub static RULE_SET: ::conjure_rule_sets::RuleSet<'static> = ::conjure_rule_sets::RuleSet::new(#name, &[#(#dependencies),*]);
    };

    TokenStream::from(expanded)
}
