use proc_macro::TokenStream;

use proc_macro2::Span;
use quote::quote;
use syn::token::Comma;
use syn::{
    ExprClosure, Ident, ItemFn, LitInt, LitStr, Result, Token, bracketed, parenthesized,
    parse::Parse, parse::ParseStream, parse_macro_input,
};

/// Parsed arguments for `#[register_rule(...)]`.
///
/// The attribute syntax is:
///
/// ```text
/// #[register_rule("RuleSet", priority)]
/// #[register_rule(["RuleSetA", "RuleSetB"], priority)]
/// #[register_rule("RuleSet", priority, [prefilter, ...])]
/// ```
///
/// The optional prefilter list narrows where the scheduler attempts the rule:
///
/// ```text
/// Variant              // focused expression must have this Expression variant
/// * / Variant          // focused expression must have an immediate child with this variant
/// Variant / Variant    // focused expression must have the left variant and an immediate child
///                      // with the right variant
/// Atomic / Reference   // focused expression must be an atomic reference
/// Atomic / Literal     // focused expression must be an atomic literal
/// ```
///
/// Items in the list are alternatives. For example, `[And / Comprehension, Or / Comprehension]`
/// means `(And with direct Comprehension child) OR (Or with direct Comprehension child)`.
///
/// Omitting the prefilter list makes the rule universal. A universal rule is considered at every
/// focused expression for its priority. In Rust source, write slash filters with spaces around the
/// slash, e.g. `[* / Bubble]` or `[Atomic / Reference]`, to keep the syntax visually distinct.
enum ParsedPrefilter {
    /// Focused-expression variant prefilter, e.g. `Add` or `Sub`.
    Variant(Ident),
    /// Immediate-child variant prefilter, parsed from `* / Child`.
    Child { child: Ident },
    /// Paired focused-expression and immediate-child variant prefilter, parsed from `Root / Child`.
    VariantChild { variant: Ident, child: Ident },
    /// Atomic subvariant prefilter, parsed from `Atomic / Reference` or `Atomic / Literal`.
    Atom(Ident),
}

struct RegisterRuleArgs {
    /// Rule set names this rule belongs to.
    rule_sets: Vec<LitStr>,
    /// Priority used for every listed rule set.
    priority: LitInt,
    /// Complete prefilter alternatives. Empty means the rule is universal.
    prefilters: Vec<ParsedPrefilter>,
}

impl Parse for RegisterRuleArgs {
    fn parse(input: ParseStream) -> Result<Self> {
        if input.is_empty() {
            return Ok(RegisterRuleArgs {
                rule_sets: Vec::new(),
                priority: LitInt::new("0", Span::call_site()),
                prefilters: Vec::new(),
            });
        }

        let rule_sets = if input.peek(syn::token::Bracket) {
            let content;
            bracketed!(content in input);

            let mut rule_sets = Vec::new();
            while !content.is_empty() {
                let rule_set: LitStr = content.parse()?;
                rule_sets.push(rule_set);
                if content.is_empty() {
                    break;
                }
                let _: Comma = content.parse()?;
            }
            rule_sets
        } else {
            vec![input.parse()?]
        };

        let _: Comma = input.parse()?;
        let priority: LitInt = input.parse()?;

        // Parse optional variant names in brackets: "Minion", 4200, [Add, Sub]
        let mut prefilters = Vec::new();
        if input.peek(Comma) {
            let _: Comma = input.parse()?;
            let content;
            bracketed!(content in input);
            while !content.is_empty() {
                if content.peek(Token![*]) {
                    let _: Token![*] = content.parse()?;
                    let _: Token![/] = content.parse()?;
                    let variant: Ident = content.parse()?;
                    prefilters.push(ParsedPrefilter::Child { child: variant });
                } else {
                    let variant: Ident = content.parse()?;
                    if content.peek(Token![/]) {
                        let _: Token![/] = content.parse()?;
                        let subvariant: Ident = content.parse()?;
                        if variant != "Atomic" {
                            prefilters.push(ParsedPrefilter::VariantChild {
                                variant,
                                child: subvariant,
                            });
                        } else {
                            match subvariant.to_string().as_str() {
                                "Literal" | "Reference" => {
                                    prefilters.push(ParsedPrefilter::Atom(subvariant));
                                }
                                _ => {
                                    return Err(syn::Error::new(
                                        subvariant.span(),
                                        "expected Literal or Reference after Atomic /",
                                    ));
                                }
                            }
                        }
                    } else {
                        prefilters.push(ParsedPrefilter::Variant(variant));
                    }
                }
                if content.is_empty() {
                    break;
                }
                let _: Comma = content.parse()?;
            }
        }

        Ok(RegisterRuleArgs {
            rule_sets,
            priority,
            prefilters,
        })
    }
}

/// Register a rule with the given rule sets and priorities.
#[proc_macro_attribute]
pub fn register_rule(arg_tokens: TokenStream, item: TokenStream) -> TokenStream {
    let func = parse_macro_input!(item as ItemFn);
    let rule_ident = &func.sig.ident;
    let static_name = format!("CONJURE_GEN_RULE_{rule_ident}").to_uppercase();
    let static_ident = Ident::new(&static_name, rule_ident.span());

    let args = parse_macro_input!(arg_tokens as RegisterRuleArgs);

    let rule_sets_token = if args.rule_sets.is_empty() {
        quote! { &[] }
    } else {
        let rule_sets = &args.rule_sets;
        let priority = &args.priority;
        quote! { &[#((#rule_sets, #priority as u16)),*] }
    };

    let prefilters = if args.prefilters.is_empty() {
        quote! { None }
    } else {
        let prefilter_tokens = args.prefilters.iter().map(|prefilter| match prefilter {
            ParsedPrefilter::Variant(variant) => {
                quote! {
                    ::conjure_cp::rule_engine::RulePrefilter::Variant(
                        ::conjure_cp::discriminant_from_name!(#variant)
                    )
                }
            }
            ParsedPrefilter::Child { child } => {
                quote! {
                    ::conjure_cp::rule_engine::RulePrefilter::Child {
                        child: ::conjure_cp::discriminant_from_name!(#child),
                    }
                }
            }
            ParsedPrefilter::VariantChild { variant, child } => {
                quote! {
                    ::conjure_cp::rule_engine::RulePrefilter::VariantChild {
                        variant: ::conjure_cp::discriminant_from_name!(#variant),
                        child: ::conjure_cp::discriminant_from_name!(#child),
                    }
                }
            }
            ParsedPrefilter::Atom(atom_variant) => {
                quote! {
                    ::conjure_cp::rule_engine::RulePrefilter::Atom(
                        ::conjure_cp::rule_engine::AtomKind::#atom_variant
                    )
                }
            }
        });
        quote! {
            Some(&[#(#prefilter_tokens),*])
        }
    };

    let expanded = quote! {
        #func

        use ::conjure_cp::rule_engine::_dependencies::*; // ToDo idk if we need to explicitly do that?

        #[::conjure_cp::rule_engine::_dependencies::distributed_slice(::conjure_cp::rule_engine::RULES_DISTRIBUTED_SLICE)]
        pub static #static_ident: ::conjure_cp::rule_engine::Rule<'static> = ::conjure_cp::rule_engine::Rule {
            name: stringify!(#rule_ident),
            application: #rule_ident,
            rule_sets: #rule_sets_token,
            prefilters: #prefilters,
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
