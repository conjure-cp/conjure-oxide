use proc_macro::TokenStream;

use proc_macro2::Span;
use quote::quote;
use syn::punctuated::Punctuated;
use syn::token::Comma;
use syn::{
    ExprClosure, Ident, ItemFn, LitInt, LitStr, Result, parenthesized, parse::Parse,
    parse::ParseStream, parse_macro_input,
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

/**
 * Register a rule with the given rule sets and priorities.
 */
#[proc_macro_attribute]
pub fn register_rule(arg_tokens: TokenStream, item: TokenStream) -> TokenStream {
    let func = parse_macro_input!(item as ItemFn);
    let rule_ident = &func.sig.ident;
    let static_name = format!("CONJURE_GEN_RULE_{rule_ident}").to_uppercase();
    let static_ident = Ident::new(&static_name, rule_ident.span());

    let args = parse_macro_input!(arg_tokens as RegisterRuleArgs);
    let rule_sets = args
        .rule_sets
        .iter()
        .map(|rule_set| {
            let rule_set_name = &rule_set.rule_set;
            let priority = &rule_set.priority;
            quote! {
                (#rule_set_name, #priority as u16)
            }
        })
        .collect::<Vec<_>>();

    let expanded = quote! {
        #func

        use ::conjure_cp::rule_engine::_dependencies::*; // ToDo idk if we need to explicitly do that?

        #[::conjure_cp::rule_engine::_dependencies::distributed_slice(::conjure_cp::rule_engine::RULES_DISTRIBUTED_SLICE)]
        pub static #static_ident: ::conjure_cp::rule_engine::Rule<'static> = ::conjure_cp::rule_engine::Rule {
            name: stringify!(#rule_ident),
            application: #rule_ident,
            rule_sets: &[#(#rule_sets),*],
        };
    };

    TokenStream::from(expanded)
}

fn parse_parenthesized<T: Parse>(input: ParseStream) -> Result<Vec<T>> {
    let content;
    parenthesized!(content in input);

    let mut paths = Vec::new();
    while !content.is_empty() {
        let path = content.parse()?;
        paths.push(path);
        if content.is_empty() {
            break;
        }
        content.parse::<Comma>()?;
    }

    Ok(paths)
}

struct RuleSetArgs {
    name: LitStr,
    dependencies: Vec<LitStr>,
    applies_fn: Option<ExprClosure>,
}

impl Parse for RuleSetArgs {
    fn parse(input: ParseStream) -> Result<Self> {
        let name = input.parse()?;

        if input.is_empty() {
            return Ok(Self {
                name,
                dependencies: Vec::new(),
                applies_fn: None,
            });
        }

        input.parse::<Comma>()?;
        let dependencies = parse_parenthesized::<LitStr>(input)?;

        if input.is_empty() {
            return Ok(Self {
                name,
                dependencies,
                applies_fn: None,
            });
        }

        input.parse::<Comma>()?;
        let applies_fn = input.parse::<ExprClosure>()?;

        Ok(Self {
            name,
            dependencies,
            applies_fn: Some(applies_fn),
        })
    }
}

/**
* Register a rule set with the given name, dependencies, and metadata.
*
* # Example
* ```rust
 * use conjure_cp_rule_macros::register_rule_set;
 * register_rule_set!("MyRuleSet", ("DependencyRuleSet", "AnotherRuleSet"));
* ```
 */
#[proc_macro]
pub fn register_rule_set(args: TokenStream) -> TokenStream {
    let RuleSetArgs {
        name,
        dependencies,
        applies_fn,
    } = parse_macro_input!(args as RuleSetArgs);

    let static_name = format!("CONJURE_GEN_RULE_SET_{}", name.value()).to_uppercase();
    let static_ident = Ident::new(&static_name, Span::call_site());

    let dependencies = quote! {
        #(#dependencies),*
    };

    let applies_to_family = match applies_fn {
        // Does not apply by default, e.g. only used as a dependency
        None => quote! { |_: &::conjure_cp::settings::SolverFamily| false },
        Some(func) => quote! { #func },
    };

    let expanded = quote! {
        use ::conjure_cp::rule_engine::_dependencies::*; // ToDo idk if we need to explicitly do that?
        #[::conjure_cp::rule_engine::_dependencies::distributed_slice(::conjure_cp::rule_engine::RULE_SETS_DISTRIBUTED_SLICE)]
        pub static #static_ident: ::conjure_cp::rule_engine::RuleSet<'static> =
            ::conjure_cp::rule_engine::RuleSet::new(#name, &[#dependencies], #applies_to_family);
    };

    TokenStream::from(expanded)
}
