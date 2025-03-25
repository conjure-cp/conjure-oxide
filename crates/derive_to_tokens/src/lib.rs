mod util;

use crate::util::add_bounds;
use proc_macro;
use proc_macro2::TokenStream;
use quote::{format_ident, quote};
use syn::spanned::Spanned;
use syn::{parse_quote, FieldsUnnamed};
use syn::{Data, DataEnum, DataStruct, DeriveInput, Error, Fields, Result};
use syn::{Ident, Index};

#[proc_macro_derive(ToTokens, attributes(to_tokens))]
pub fn derive_to_tokens(item: proc_macro::TokenStream) -> proc_macro::TokenStream {
    let res = expand(item.into()).unwrap_or_else(|err| err.to_compile_error());
    res.into()
}

/// Actual implementation of `#[derive(ToTokens)]`.
fn expand(stream: TokenStream) -> Result<TokenStream> {
    let item: DeriveInput = syn::parse2(stream)?;
    let ty_name = item.ident.clone();

    let body = match &item.data {
        Data::Union(_) => return Err(Error::new_spanned(&item, "unions are not supported")),
        Data::Enum(data) => expand_enum(&ty_name, data)?,
        Data::Struct(data) => todo!("Implement struct expansion"),
    };
    let generics = item.generics.clone();
    let (impl_gen, ty_gen, where_clause) = generics.split_for_impl();
    let bounds = parse_quote!(::quote::ToTokens);
    let where_clause = add_bounds(item, where_clause, bounds)?;

    Ok(quote! {
        #[automatically_derived]
        #[allow(non_snake_case)]
        impl #impl_gen ::quote::ToTokens for #ty_name #ty_gen #where_clause {
            fn to_tokens(&self, tokens: &mut ::proc_macro2::TokenStream) {
                #body
            }
        }
    })
}

fn expand_enum(ty_name: &Ident, data: &DataEnum) -> Result<TokenStream> {
    let body = expand_variants(ty_name, data)?;

    Ok(quote! {
        match self {
            #body
        }
    })
}

/// Generates a `match` so that the fields of the currently active variant
/// will be appended to the token stream.
fn expand_variants(ty_name: &Ident, data: &DataEnum) -> Result<TokenStream> {
    let arms: Vec<TokenStream> = data.variants.iter().map(|variant| {
        let variant_name = &variant.ident;
        match &variant.fields {
            Fields::Unit => {
                // For unit variants, there is nothing to bind.
                Ok(quote! {
                    #ty_name::#variant_name => {
                        tokens.extend(quote! { #ty_name::#variant_name });
                    }
                })
            },
            Fields::Unnamed(fields) => {
                // For tuple variants, generate bindings for each field.
                let field_bindings: Vec<Ident> =
                    (0..fields.unnamed.len()).map(|i| format_ident!("field_{}", i)).collect();
                Ok(quote! {
                    #ty_name::#variant_name ( #( ref #field_bindings ),* ) => {
                        tokens.extend(quote! { #ty_name::#variant_name ( #(##field_bindings.into()),* ) });
                    }
                })
            },
            Fields::Named(fields) => {
                // For named variants, use the actual field names.
                // Since we're in a named variant, each field is expected to have an identifier.
                let field_idents: Vec<Ident> = fields.named.iter()
                    .map(|f| f.ident.clone().expect("named variant must have field names"))
                    .collect();
                Ok(quote! {
                    #ty_name::#variant_name { #( ref #field_idents, )* } => {
                        tokens.extend(quote! { #ty_name::#variant_name { #( #field_idents: ##field_idents.into() ),* } });
                    }
                })
            },
        }
    }).collect::<Result<Vec<_>>>()?;

    Ok(quote! {
        #(#arms)*
    })
}

// fn expand_fields_unnamed(fields: &FieldsUnnamed) -> Result<TokenStream> {
//     let field_bindings: Vec<Ident> = (0..fields.unnamed.len()).map(|i| format_ident!("field_{}", i)).collect();
//     let field_values =
//     // Ok(quote! {
//     //     #ty_name::#variant_name ( #( ref #field_bindings ),* ) => {
//     //         tokens.extend(quote! { #ty_name::#variant_name ( #(##field_bindings.into()),* ) });
//     //     }
//     // })
// }
