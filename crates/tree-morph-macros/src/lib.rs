use proc_macro::TokenStream;
use quote::quote;
use syn::{parse_macro_input, FnArg, GenericArgument, ItemFn, PathArguments, Type};


/// Creates a named rule wrapper function for tree-morph rules.
///
/// This macro transforms a rule function to return an `impl Rule<T, M>`
/// by wrapping it in a `NamedRule`.
///
///  ```
/// #[named_rule("CustomName")]
/// fn my_rule(_: &mut Commands<Expr, Meta>, expr: &Expr, _: &Meta) -> Option<Expr> {
///     // rule implementation
/// }
/// ```
///
/// The above function will be transformed to return `impl Rule<Expr, Meta>` and
/// have its name set to "CustomName". If not specified, the name will simply be
/// the function identifier. In the above case it will be "my_rule".
///
/// The original function logic is preserved in a private helper function.
#[proc_macro_attribute]
pub fn named_rule(attr: TokenStream, item: TokenStream) -> TokenStream {
    let function = parse_macro_input!(item as ItemFn);
    let name = &function.sig.ident;
    let vis = &function.vis;

    let rule_name = if attr.is_empty() {
        name.to_string()
    } else {
        let name_lit: syn::LitStr = parse_macro_input!(attr as syn::LitStr);
        name_lit.value()
    };

    // Create a private helper function with the original implementation
    let helper_name = syn::Ident::new(&format!("__{}_impl", name), name.span());
    let mut helper_function = function.clone();
    helper_function.sig.ident = helper_name.clone();
    helper_function.vis = syn::Visibility::Inherited; // Make it private

    let commmand_param = &function.sig.inputs[0];
    let (type_t, type_m) = extract_commands_types(commmand_param);

    let expanded = quote! {
        #helper_function

        #vis fn #name() -> ::tree_morph::prelude::NamedRule<::tree_morph::prelude::RuleFn<#type_t, #type_m>> {
            ::tree_morph::prelude::NamedRule::new(#rule_name, #helper_name as ::tree_morph::prelude::RuleFn<#type_t, #type_m>)
        }
    };

    TokenStream::from(expanded)
}

fn extract_commands_types(param: &syn::FnArg) -> (syn::Type, syn::Type) {
    let FnArg::Typed(pat_type) = param else {
        panic!("Expected typed parameter");
    };
    
    let Type::Reference(type_ref) = &*pat_type.ty else {
        panic!("Expected reference type");
    };
    
    let Type::Path(type_path) = &*type_ref.elem else {
        panic!("Expected path type");
    };
    
    let segment = type_path.path.segments.last().unwrap();
    
    let PathArguments::AngleBracketed(args) = &segment.arguments else {
        panic!("Commands must have type parameters");
    };
    
    let GenericArgument::Type(t) = &args.args[0] else {
        panic!("First argument must be a type");
    };
    
    let GenericArgument::Type(m) = &args.args[1] else {
        panic!("Second argument must be a type");
    };
    
    (t.clone(), m.clone())
}
