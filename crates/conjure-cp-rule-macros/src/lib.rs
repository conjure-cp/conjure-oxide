use proc_macro::TokenStream;

use proc_macro2::Span;
use quote::quote;
use syn::token::Comma;
use syn::{
    ExprClosure, Ident, ItemFn, LitInt, LitStr, Result, parenthesized, parse::Parse,
    parse::ParseStream, parse_macro_input,
};

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

struct RegisterRuleArgs {
    pub rule_sets: Vec<RuleSetAndPriority>,
    pub applicable_patterns: Option<proc_macro2::TokenStream>,
}

impl Parse for RegisterRuleArgs {
    fn parse(input: ParseStream) -> Result<Self> {
        let mut rule_sets = Vec::new();
        let mut applicable_patterns = None;

        while !input.is_empty() {
            // Check if the next token is the `applicable_to` keyword
            if input.peek(Ident) {
                let fork = input.fork();
                let ident: Ident = fork.parse()?;
                if ident == "applicable_to" {
                    // Consume the identifier from the real stream
                    let _: Ident = input.parse()?;
                    let content;
                    parenthesized!(content in input);
                    applicable_patterns = Some(content.parse::<proc_macro2::TokenStream>()?);
                    break;
                }
            }

            // Otherwise parse as a rule set + priority tuple
            rule_sets.push(input.parse::<RuleSetAndPriority>()?);
            if !input.is_empty() {
                input.parse::<Comma>()?;
            }
        }

        Ok(RegisterRuleArgs {
            rule_sets,
            applicable_patterns,
        })
    }
}

/**
 * Register a rule with the given rule sets and priorities.
 *
 * Optionally specify which expression variants the rule can apply to for fast pre-filtering:
 * ```ignore
 * #[register_rule(("Base", 8800), applicable_to(Expression::Not(..) | Expression::And(..)))]
 * ```
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

    let (can_apply_fn_def, can_apply_field) = match &args.applicable_patterns {
        Some(patterns) => {
            let fn_name = Ident::new(
                &format!("__can_apply_{}", rule_ident),
                rule_ident.span(),
            );
            let def = quote! {
                fn #fn_name(expr: &::conjure_cp::ast::Expression) -> bool {
                    matches!(expr, #patterns)
                }
            };
            let field = quote! { Some(#fn_name) };
            (def, field)
        }
        None => {
            (quote! {}, quote! { None })
        }
    };

    let expanded = quote! {
        #func

        use ::conjure_cp::rule_engine::_dependencies::*; // ToDo idk if we need to explicitly do that?

        #can_apply_fn_def

        #[::conjure_cp::rule_engine::_dependencies::distributed_slice(::conjure_cp::rule_engine::RULES_DISTRIBUTED_SLICE)]
        pub static #static_ident: ::conjure_cp::rule_engine::Rule<'static> = ::conjure_cp::rule_engine::Rule {
            name: stringify!(#rule_ident),
            application: #rule_ident,
            rule_sets: &[#(#rule_sets),*],
            can_apply: #can_apply_field,
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
