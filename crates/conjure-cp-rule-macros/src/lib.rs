mod util;

use proc_macro::TokenStream;
use proc_macro2::Span;
use quote::{ToTokens, quote};
use std::collections::HashMap;
use syn::punctuated::Punctuated;
use syn::token::Comma;
use syn::{
    ExprClosure, GenericParam, Ident, ItemFn, ItemImpl, ItemStruct, LitInt, LitStr, Result,
    TypePath, parenthesized, parse::Parse, parse::ParseStream, parse_macro_input,
};

use crate::util::rename_ident_in_impl;
use util::{rename_fn, rename_ident_in_fn, replace_ident_in_type, type_contains_ident};

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

enum ReprStateType {
    Struct(ItemStruct, Option<ItemImpl>),
    Path(TypePath),
}

impl Parse for ReprStateType {
    fn parse(input: ParseStream) -> Result<Self> {
        if let Ok(strct) = input.parse::<ItemStruct>() {
            let imp = input.parse::<ItemImpl>().ok();
            return Ok(ReprStateType::Struct(strct, imp));
        }
        if let Ok(path) = input.parse::<TypePath>() {
            return Ok(ReprStateType::Path(path));
        }
        Err(input.error(
            "Expected one of:\
        - A struct definition, e.g `struct State<T> {...}`\
        - A path to an existing type, e.g `Vec<T>`\
        ",
        ))
    }
}

struct ReprDefArgs {
    /// Name of this representation rule
    ident: Ident,
    /// Representation state type
    state_ty: ReprStateType,
    /// Initialisation at domain level: `init: (DomainPtr) -> Result<State<DomainPtr>, ReprError>`
    init_fn: ItemFn,
    /// Generating structural constraints: `structural: (&State<DeclarationPtr>) -> Vec<Expression>`
    structural_fn: ItemFn,
    /// Going down: `down: (&State<DeclarationPtr>, Literal) -> Result<State<Literal>, ReprError>`
    down_fn: ItemFn,
    /// Going up: `up: State<Literal> -> Literal`
    up_fn: ItemFn,
}

impl Parse for ReprDefArgs {
    fn parse(input: ParseStream) -> Result<Self> {
        let ident = input.parse::<Ident>()?;
        // input.parse::<Comma>()?;
        let state_ty = input.parse::<ReprStateType>()?;

        // TODO: Exact syntax subject to change

        let mut funcs = HashMap::<String, ItemFn>::new();
        for _ in 0..4 {
            // input.parse::<Comma>()?;
            let func = input.parse::<ItemFn>()?;
            let ident = func.sig.ident.to_string();
            funcs.insert(ident, func);
        }

        let init_fn = funcs.remove("init").ok_or_else(|| {
            input.error("Expected `fn init(DomainPtr) -> Result<State<DomainPtr>, ReprError>`")
        })?;
        let structural_fn = funcs.remove("structural").ok_or_else(|| {
            input.error("Expected `fn structural(&State<DeclarationPtr>) -> Vec<Expression>`")
        })?;
        let down_fn = funcs.remove("down").ok_or_else(|| input.error("Expected `fn down(&State<DomainPtr>, Literal) -> Result<State<Literal>, ReprError>`"))?;
        let up_fn = funcs
            .remove("up")
            .ok_or_else(|| input.error("Expected `fn up(State<Literal>) -> Literal`"))?;

        Ok(Self {
            ident,
            state_ty,
            init_fn,
            structural_fn,
            down_fn,
            up_fn,
        })
    }
}

#[proc_macro]
pub fn register_representation(input: TokenStream) -> TokenStream {
    let args = parse_macro_input!(input as ReprDefArgs);
    let repr_ident = &args.ident;
    let repr_name_str = repr_ident.to_string();

    // prefix for generated names
    let prefix = format!("CONJURE_GEN_REPR_{}_", repr_name_str);

    let (user_state_ident, struct_def_tokens) = match &args.state_ty {
        ReprStateType::Struct(item_struct, _) => {
            // get ident and body of the struct
            let ident = item_struct.ident.clone();
            let prefixed_ident =
                Ident::new(&format!("{}{}", prefix, ident), item_struct.ident.span());
            let tokens = generate_struct_def(item_struct, &prefixed_ident);
            (ident, tokens)
        }
        ReprStateType::Path(type_path) => {
            // for a path like `foo::MyState`, just use it directly; no struct to emit
            let ident = type_path
                .path
                .segments
                .last()
                .expect("state type path must have at least one segment")
                .ident
                .clone();
            (ident, quote! {})
        }
    };

    // Actual ident of the "State<T>" type
    let state_ident = match &args.state_ty {
        ReprStateType::Struct(..) => {
            // prefix user-defined struct's ident so it doesn't clash with anything
            Ident::new(
                &format!("{}{}", prefix, user_state_ident),
                user_state_ident.span(),
            )
        }
        // otherwise use the provided name as is
        ReprStateType::Path(_) => user_state_ident.clone(),
    };

    // Rename the idents in user-defined functions to their prefixed versions
    // e.g:
    // MyState<T> -> CONJURE_GEN_REPR_<Rule>_MyState<T>
    // fn init(...) -> fn CONJURE_GEN_REPR_<Rule>_init(...)
    let prefixed_init = Ident::new(&format!("{}init", prefix), args.init_fn.sig.ident.span());
    let prefixed_structural = Ident::new(
        &format!("{}structural", prefix),
        args.structural_fn.sig.ident.span(),
    );
    let prefixed_down = Ident::new(&format!("{}down", prefix), args.down_fn.sig.ident.span());
    let prefixed_up = Ident::new(&format!("{}up", prefix), args.up_fn.sig.ident.span());

    let mut init_fn = rename_fn(args.init_fn, &prefixed_init);
    let mut structural_fn = rename_fn(args.structural_fn, &prefixed_structural);
    let mut down_fn = rename_fn(args.down_fn, &prefixed_down);
    let mut up_fn = rename_fn(args.up_fn, &prefixed_up);

    if matches!(&args.state_ty, ReprStateType::Struct(..)) {
        init_fn = rename_ident_in_fn(init_fn, &user_state_ident, &state_ident);
        structural_fn = rename_ident_in_fn(structural_fn, &user_state_ident, &state_ident);
        down_fn = rename_ident_in_fn(down_fn, &user_state_ident, &state_ident);
        up_fn = rename_ident_in_fn(up_fn, &user_state_ident, &state_ident);
    }

    // Rename idents in the user-provided impl
    let renamed_impl = if let ReprStateType::Struct(_, Some(item_impl)) = args.state_ty {
        Some(rename_ident_in_impl(
            item_impl,
            &user_state_ident,
            &state_ident,
        ))
    } else {
        None
    };

    // Static name for distributed_slice entry
    let static_name = format!("CONJURE_GEN_REPR_{}", repr_name_str).to_uppercase();
    let static_ident = Ident::new(&static_name, repr_ident.span());

    let expanded = quote! {
        // -- Dependencies
        use ::conjure_cp::representation::_dependencies::*;
        use ::conjure_cp::ast::{
            DeclarationPtr, DomainPtr, Expression, Literal, SymbolTable, Name
        };

        // -- User-provided struct definition
        #struct_def_tokens

        // -- User-provided struct impl
        #renamed_impl

        // -- User-provided functions
        #[allow(non_snake_case)]
        #init_fn
        #[allow(non_snake_case)]
        #structural_fn
        #[allow(non_snake_case)]
        #down_fn
        #[allow(non_snake_case)]
        #up_fn

        // -- Trait implementations
        impl ReprDomainLevel for #state_ident<DomainPtr> {
            type Assignment = #state_ident<Literal>;
            type DeclLevel = #state_ident<DeclarationPtr>;

            fn init(dom: DomainPtr) -> ::core::result::Result<Self, ReprError>
            where
                Self: Sized,
            {
                #prefixed_init(dom)
            }

            fn down(
                &self,
                value: Literal,
            ) -> ::core::result::Result<Self::Assignment, ReprError> {
                #prefixed_down(self, value)
            }

            fn instantiate(self, decl: DeclarationPtr) -> (Self::DeclLevel, SymbolTable, Vec<Expression>) {
                instantiate_default_impl(self, decl, #repr_name_str, #prefixed_structural)
            }
        }

        impl ReprDeclLevel for #state_ident<DeclarationPtr> {
            type Assignment = #state_ident<Literal>;
            type DomainLevel = #state_ident<DomainPtr>;

            fn to_domain_level(self) -> Self::DomainLevel {
                let field_dom = |decl: DeclarationPtr| {
                    decl.domain().expect("variable must have a domain")
                };
                self.func_map(field_dom)
            }

            fn lookup_via(
                &self,
                lookup: &LookupFn<'_>,
            ) -> ::core::result::Result<Self::Assignment, ReprError> {
                self.clone().try_func_map(|decl: DeclarationPtr| try_up_via(decl, lookup))
            }
        }

        impl ReprAssignment for #state_ident<Literal> {
            fn up(self) -> Literal {
                #prefixed_up(self)
            }
        }

        // -- ReprRule marker struct
        pub struct #repr_ident;

        impl ReprRule for #repr_ident {
            const NAME: &'static str = #repr_name_str;
            type Assignment = #state_ident<Literal>;
            type DeclLevel = #state_ident<DeclarationPtr>;
            type DomainLevel = #state_ident<DomainPtr>;
        }

        // -- Registry entry
        #[::conjure_cp::representation::_dependencies::distributed_slice(::conjure_cp::representation::_dependencies::REPR_RULES_DISTRIBUTED_SLICE)]
        pub static #static_ident: ::conjure_cp::representation::_dependencies::ReprRegistryEntry = ::conjure_cp::representation::_dependencies::ReprRegistryEntry::from_rule::<#repr_ident>();
    };

    TokenStream::from(expanded)
}

/// Generates the struct definition with the necessary derive macros and serde attributes.
/// The struct is always emitted as `pub` with the given `prefixed_ident` as its name.
fn generate_struct_def(
    item_struct: &ItemStruct,
    prefixed_ident: &Ident,
) -> proc_macro2::TokenStream {
    // Find the generic type parameter name (e.g. `T`)
    let generic_param_ident = item_struct
        .generics
        .params
        .iter()
        .find_map(|p| {
            if let GenericParam::Type(tp) = p {
                Some(tp.ident.clone())
            } else {
                None
            }
        })
        .expect("state struct must have exactly one type parameter");

    let generics = &item_struct.generics;

    let serde_bound = format!(
        "ReprStateSerde: SerializeAs<{0}> + for<'d> DeserializeAs<'d, {0}>",
        generic_param_ident
    );

    let fields = match &item_struct.fields {
        syn::Fields::Named(named) => {
            let field_tokens: Vec<_> = named
                .named
                .iter()
                .map(|f| {
                    let field_attrs = &f.attrs;
                    let field_vis = &f.vis;
                    let field_ident = &f.ident;
                    let field_ty = &f.ty;

                    if type_contains_ident(&f.ty, &generic_param_ident) {
                        let serde_as_ty = replace_ident_in_type(
                            f.ty.clone(),
                            &generic_param_ident,
                            "ReprStateSerde",
                        );
                        let serde_as_str = serde_as_ty.to_token_stream().to_string();
                        quote! {
                            #(#field_attrs)*
                            #[serde_as(as = #serde_as_str)]
                            #field_vis #field_ident: #field_ty
                        }
                    } else {
                        quote! {
                            #(#field_attrs)*
                            #field_vis #field_ident: #field_ty
                        }
                    }
                })
                .collect();

            quote! { { #(#field_tokens),* } }
        }
        syn::Fields::Unnamed(unnamed) => {
            let field_tokens: Vec<_> = unnamed
                .unnamed
                .iter()
                .map(|f| {
                    let field_attrs = &f.attrs;
                    let field_vis = &f.vis;
                    let field_ty = &f.ty;

                    if type_contains_ident(&f.ty, &generic_param_ident) {
                        let serde_as_ty = replace_ident_in_type(
                            f.ty.clone(),
                            &generic_param_ident,
                            "ReprStateSerde",
                        );
                        let serde_as_str = serde_as_ty.to_token_stream().to_string();
                        quote! {
                            #(#field_attrs)*
                            #[serde_as(as = #serde_as_str)]
                            #field_vis #field_ty
                        }
                    } else {
                        quote! {
                            #(#field_attrs)*
                            #field_vis #field_ty
                        }
                    }
                })
                .collect();

            quote! { ( #(#field_tokens),* ); }
        }
        syn::Fields::Unit => quote! { ; },
    };

    quote! {
        #[allow(non_camel_case_types)]
        #[::conjure_cp::representation::_dependencies::serde_with::serde_as(
            crate = "::conjure_cp::representation::_dependencies::serde_with"
        )]
        #[derive(
            Debug,
            Clone,
            PartialEq,
            Eq,
            ::conjure_cp::representation::_dependencies::funcmap::FuncMap,
            ::conjure_cp::representation::_dependencies::funcmap::TryFuncMap,
            ::conjure_cp::representation::_dependencies::serde::Serialize,
            ::conjure_cp::representation::_dependencies::serde::Deserialize
        )]
        #[serde(
            crate = "::conjure_cp::representation::_dependencies::serde",
            bound = #serde_bound
        )]
        #[funcmap(crate = "::conjure_cp::representation::_dependencies::funcmap")]
        pub struct #prefixed_ident #generics #fields
    }
}
