use proc_macro::{self, TokenStream};

use proc_macro2::TokenStream as TokenStream2;
use quote::{format_ident, quote};
use syn::{parse_macro_input, Data, DataEnum, DeriveInput, Ident, Variant};

use crate::utils::generate::{generate_field_clones, generate_field_fills, generate_field_idents};

mod utils;

/// Generate the full match pattern for a variant
fn generate_match_pattern(variant: &Variant, root_ident: &Ident) -> TokenStream2 {
    let field_idents = generate_field_idents(&variant.fields);
    let variant_ident = &variant.ident;

    if field_idents.is_empty() {
        quote! {
            #root_ident::#variant_ident
        }
    } else {
        quote! {
            #root_ident::#variant_ident(#(#field_idents,)*)
        }
    }
}

/// Generate the code to get the children of a variant
fn generate_variant_children_match_arm(variant: &Variant, root_ident: &Ident) -> TokenStream2 {
    let field_clones = generate_field_clones(&variant.fields, root_ident);

    let match_pattern = generate_match_pattern(variant, root_ident);

    let clones = if field_clones.is_empty() {
        quote! {
            Vec::new()
        }
    } else {
        quote! {
            vec![#(#field_clones,)*].iter().flatten().cloned().collect::<Vec<_>>()
        }
    };

    let mach_arm = quote! {
         #match_pattern => {
            #clones
        }
    };

    mach_arm
}

/// Generate an implementation of `context` for a variant
fn generate_variant_context_match_arm(variant: &Variant, root_ident: &Ident) -> TokenStream2 {
    let variant_ident = &variant.ident;
    let children_ident = Ident::new("children", variant_ident.span());
    let field_fills = generate_field_fills(&variant.fields, root_ident, &children_ident);
    let error_ident = format_ident!("UniplateError{}", root_ident);
    let match_pattern = generate_match_pattern(variant, root_ident);

    if field_fills.is_empty() {
        quote! {
            #match_pattern => {
                Box::new(|_| Ok(#root_ident::#variant_ident))
            }
        }
    } else {
        quote! {
            #match_pattern => {
                Box::new(|children| {
                    if (children.len() != self.children().len()) {
                        return Err(#error_ident::WrongNumberOfChildren(self.children().len(), children.len()));
                    }

                    let mut #children_ident = children.clone();
                    Ok(#root_ident::#variant_ident(#(#field_fills,)*))
                })
            }
        }
    }
}

/// Derive the `Uniplate` trait for an arbitrary type
///
/// # WARNING
///
/// This is alpha code. It is not yet stable and some features are missing.
///
/// ## What works?
///
/// - Deriving `Uniplate` for enum types
/// - `Box<T>` and `Vec<T>` fields, including nested vectors
/// - Tuple fields, including nested tuples - e.g. `(Vec<T>, (Box<T>, i32))`
///
/// ## What does not work?
///
/// - Structs
/// - Unions
/// - Array fields
/// - Multiple type arguments - e.g. `MyType<T, R>`
/// - Any complex type arguments, e.g. `MyType<T: MyTrait1 + MyTrait2>`
/// - Any collection type other than `Vec`
/// - Any box type other than `Box`
///
/// # Usage
///
/// This macro is intended to replace a hand-coded implementation of the `Uniplate` trait.
/// Example:
///
/// ```rust
/// use uniplate_derive::Uniplate;
/// use uniplate::uniplate::Uniplate;
///
/// #[derive(PartialEq, Eq, Debug, Clone, Uniplate)]
/// enum MyEnum {
///    A(Box<MyEnum>),
///    B(Vec<MyEnum>),
///    C(i32),
/// }
///
/// let a = MyEnum::A(Box::new(MyEnum::C(42)));
/// let (children, context) = a.uniplate();
/// assert_eq!(children, vec![MyEnum::C(42)]);
/// assert_eq!(context(vec![MyEnum::C(42)]).unwrap(), a);
/// ```
///
#[proc_macro_derive(Uniplate)]
pub fn derive(macro_input: TokenStream) -> TokenStream {
    let input = parse_macro_input!(macro_input as DeriveInput);
    let root_ident = &input.ident;
    let data = &input.data;

    let children_impl: TokenStream2 = match data {
        Data::Struct(_) => unimplemented!("Structs currently not supported"), // ToDo support structs
        Data::Union(_) => unimplemented!("Unions currently not supported"),   // ToDo support unions
        Data::Enum(DataEnum { variants, .. }) => {
            let match_arms: Vec<TokenStream2> = variants
                .iter()
                .map(|vt| generate_variant_children_match_arm(vt, root_ident))
                .collect::<Vec<_>>();

            let match_statement = quote! {
                match self {
                    #(#match_arms)*
                }
            };

            match_statement
        }
    };

    let context_impl = match data {
        Data::Struct(_) => unimplemented!("Structs currently not supported"),
        Data::Union(_) => unimplemented!("Unions currently not supported"),
        Data::Enum(DataEnum { variants, .. }) => {
            let match_arms: Vec<TokenStream2> = variants
                .iter()
                .map(|vt| generate_variant_context_match_arm(vt, root_ident))
                .collect::<Vec<_>>();

            let match_statement = quote! {
                match self {
                    #(#match_arms)*
                }
            };

            match_statement
        }
    };

    let error_ident = format_ident!("UniplateError{}", root_ident);

    let output = quote! {
        use uniplate::uniplate::UniplateError as #error_ident;

        impl Uniplate for #root_ident {
            #[allow(unused_variables)]
            fn uniplate(&self) -> (Vec<#root_ident>, Box<dyn Fn(Vec<#root_ident>) -> Result<#root_ident, #error_ident> + '_>) {
                let context: Box<dyn Fn(Vec<#root_ident>) -> Result<#root_ident, #error_ident>> = #context_impl;

                let children: Vec<#root_ident> = #children_impl;

                (children, context)
            }
        }
    };

    // println!("Final macro output:\n{}", output.to_string());

    output.into()
}
