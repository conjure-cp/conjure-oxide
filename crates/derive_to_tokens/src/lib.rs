mod util;

use crate::util::add_bounds;
use proc_macro2::TokenStream;
use quote::{format_ident, quote};
use syn::parse_quote;
use syn::spanned::Spanned;
use syn::{Data, DataEnum, DataStruct, DeriveInput, Error, Fields, Result};
use syn::{Ident, Index};

#[proc_macro_derive(ToTokens)]
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
        Data::Struct(data) => expand_struct(data)?,
    };
    let generics = item.generics.clone();
    let (impl_gen, ty_gen, where_clause) = generics.split_for_impl();
    let bounds = parse_quote!(::syn::ToTokens);
    let where_clause = add_bounds(item, where_clause, bounds)?;

    Ok(quote! {
        #[automatically_derived]
        #[allow(non_snake_case)]
        impl #impl_gen ::syn::ToTokens for #ty_name #ty_gen #where_clause {
            fn to_tokens(&self, tokens: &mut ::syn::TokenStream) {
                #body
            }
        }
    })
}

/// Prints every field in sequence, in the order they are specified in the source.
fn expand_struct(data: &DataStruct) -> Result<TokenStream> {
    match &data.fields {
        Fields::Named(fields) => fields
            .named
            .iter()
            .map(|field| {
                let field_name = field
                    .ident
                    .as_ref()
                    .ok_or_else(|| Error::new(field.span(), "unnamed field in named struct"))?;

                Ok(quote! {
                    ::syn::ToTokens::to_tokens(&self.#field_name, &mut *tokens);
                })
            })
            .collect(),
        Fields::Unnamed(fields) => fields
            .unnamed
            .iter()
            .zip(0..)
            .map(|(field, index)| {
                let span = field.span();
                let field_index = Index { index, span };

                Ok(quote! {
                    ::syn::ToTokens::to_tokens(&self.#field_index, &mut *tokens);
                })
            })
            .collect(),
        Fields::Unit => Ok(TokenStream::new()),
    }
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
    data.variants
        .iter()
        .map(|variant| {
            let variant_name = &variant.ident;
            let field_names = match &variant.fields {
                Fields::Unit => Vec::new(),
                Fields::Named(fields) => fields
                    .named
                    .iter()
                    .map(|field| {
                        field.ident.clone().ok_or_else(|| {
                            Error::new(field.span(), "unnamed field in named struct")
                        })
                    })
                    .collect::<Result<_>>()?,
                Fields::Unnamed(fields) => fields
                    .unnamed
                    .iter()
                    .enumerate()
                    .map(|(i, _)| format_ident!("syn_field_{}_{}", variant_name, i))
                    .collect(),
            };
            let bindings = match &variant.fields {
                Fields::Unit => TokenStream::new(),
                Fields::Named(_) => quote!({ #(#field_names,)* }),
                Fields::Unnamed(_) => quote! { (#(#field_names),*) },
            };

            Ok(quote! {
                #ty_name::#variant_name #bindings => {
                    #(::syn::ToTokens::to_tokens(#field_names, &mut *tokens);)*
                }
            })
        })
        .collect()
}
