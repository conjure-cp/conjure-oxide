mod util;

use crate::util::add_bounds;
use proc_macro;
use proc_macro2::{Delimiter, Group, Literal, Punct, Spacing, TokenStream, TokenTree};
use quote::{format_ident, quote, ToTokens, TokenStreamExt};
use syn::spanned::Spanned;
use syn::{parse_quote, Field, FieldsUnnamed, Type};
use syn::{Data, DataEnum, DataStruct, DeriveInput, Error, Fields, Result};
use syn::{Ident, Index};
use util::{field_wrapper, FieldWrapper};

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
                let (field_idents, field_values, extra_top) = expand_fields_unnamed(fields);
                Ok(quote! {
                    #ty_name::#variant_name ( #( ref #field_idents ),* ) => {
                        #extra_top
                        tokens.extend(quote! { #ty_name::#variant_name ( #(#field_values),* ) });
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

fn expand_fields_unnamed(fields: &FieldsUnnamed) -> (Vec<Ident>, Vec<TokenStream>, TokenStream) {
    let mut field_idents: Vec<Ident> = Vec::new();
    let mut field_values: Vec<TokenStream> = Vec::new();
    let mut extra_top: TokenStream = TokenStream::new();
    for (i, field) in fields.unnamed.iter().enumerate() {
        let ident = format_ident!("field_{}", i);
        field_idents.push(ident.clone());
        let (val, top) = expand_field(&field.ty, &ident);
        field_values.push(val);
        extra_top.extend(top);
    }
    (field_idents, field_values, extra_top)
}

fn expand_field(ty: &Type, ident: &Ident) -> (TokenStream, TokenStream) {
    match field_wrapper(ty) {
        Some(FieldWrapper::Box(inner)) => {
            // Wrap boxed values into Box::new()
            let (inner_val, inner_top) = expand_field(&inner, &ident);
            let val = quote! { Box::new(#inner_val) };
            (val, inner_top)
        }
        Some(FieldWrapper::Option(inner)) => {
            let gen_ident = format_ident!("{}_option", ident);
            let val_ident = format_ident!("{}_option_val", ident);
            let (inner_val, inner_top) = expand_field(&inner, &val_ident);
            let top = quote! {
                #inner_top
                let #gen_ident = match #ident {
                    Some(#val_ident) => quote! { Some(#inner_val) },
                    None => quote! { None },
                };
            };
            (quote! { ##gen_ident }, top)
        }
        Some(FieldWrapper::Vec(inner)) => {
            let gen_ident = format_ident!("{}_vec", ident);
            let val_ident = format_ident!("{}_vec_val", ident);
            let (inner_val, inner_top) = expand_field(&inner, &val_ident);
            let top = quote! {

                let #gen_ident = #ident.iter().map(|#val_ident: &#inner| { #inner_top quote! { #inner_val } }).collect::<Vec<_>>();
            };
            let seq = expand_sequence(&gen_ident);
            let val = quote! { vec! [ #seq ] };
            (val, top)
        }
        _ => {
            // Just insert the tokens of other types directly;
            // Add `into()` to hopefully convert the resulting literal back to the correct type
            (quote! { ##ident.into() }, TokenStream::new())
        }
    }
}

fn expand_sequence(seq: &Ident) -> TokenStream {
    // Expands to: "#(#ident),*"
    let mut inner = TokenStream::new();

    // Add "#"
    inner.append(Punct::new('#', Spacing::Alone));

    // Add parenthesized group "(#<field ident>)"
    let mut paren_content = TokenStream::new();
    paren_content.append(Punct::new('#', Spacing::Alone));
    paren_content.append(seq.clone());
    let paren_group = Group::new(Delimiter::Parenthesis, paren_content);
    inner.append(paren_group);

    // Add ",*"
    inner.append(Punct::new(',', Spacing::Alone));
    inner.append(Punct::new('*', Spacing::Alone));

    inner
}
