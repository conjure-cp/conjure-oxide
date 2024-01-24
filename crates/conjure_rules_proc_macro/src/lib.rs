use proc_macro::TokenStream;
use quote::quote;
use syn::{parse_macro_input, Ident, ItemFn};

#[doc(hidden)]
#[proc_macro_attribute]
/// This procedural macro registers a decorated function with `conjure_rules`' global registry.
/// It may be used in any downstream crate. For more information on linker magic, see the [`linkme`](https://docs.rs/linkme/latest/linkme/) crate.
///
/// **IMPORTANT**: Since the resulting rule may not be explicitly referenced, it may be removed by the compiler's dead code elimination.
/// To prevent this, you must ensure that either:
/// 1. codegen-units is set to 1, i.e. in Cargo.toml:
/// ```toml
/// [profile.release]
/// codegen-units = 1
/// ```
/// 2. The function is included somewhere else in the code
///
/// <hr>
///
/// Functions must have the signature `fn(&Expr) -> Result<Expr, RuleApplicationError>`.
/// The created rule will have the same name as the function.
///
/// Intermediary static variables are created to allow for the decentralized registry, with the prefix `CONJURE_GEN_`.
/// Please ensure that other variable names in the same scope do not conflict with these.
///
/// <hr>
///
/// For example:
/// ```rust
/// #[register_rule]
/// fn identity(expr: &Expression) -> Result<Expression, RuleApplicationError> {
///   Ok(expr.clone())
/// }
/// ```
/// ... will expand into the following code:
/// ```rust
/// fn identity(expr: &Expression) -> Result<Expression, RuleApplicationError> {
///   Ok(expr.clone())
/// }
/// #[::conjure_rules::_dependencies::distributed_slice(::conjure_rules::RULES_DISTRIBUTED_SLICE)]
/// pub static CONJURE_GEN_RULE_IDENTITY: ::conjure_rules::_dependencies::Rule = ::conjure_rules::_dependencies::Rule {
///   name: "identity",
///   application: identity,
/// };
/// ```
///
pub fn register_rule(_: TokenStream, item: TokenStream) -> TokenStream {
    let func = parse_macro_input!(item as ItemFn);
    let rule_ident = &func.sig.ident;
    let static_name = format!("CONJURE_GEN_RULE_{}", rule_ident).to_uppercase();
    let static_ident = Ident::new(&static_name, rule_ident.span());

    let expanded = quote! {
        #func

        #[::conjure_rules::_dependencies::distributed_slice(::conjure_rules::RULES_DISTRIBUTED_SLICE)]
        pub static #static_ident: ::conjure_rules::_dependencies::Rule = ::conjure_rules::_dependencies::Rule {
            name: stringify!(#rule_ident),
            application: #rule_ident,
        };
    };

    TokenStream::from(expanded)
}
