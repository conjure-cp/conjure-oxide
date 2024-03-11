mod utils;
use crate::utils::generate::{generate_field_clones, generate_field_fills, generate_field_idents};
use proc_macro::{self, TokenStream};
use proc_macro2::TokenStream as TokenStream2;
use quote::quote;
use syn::{parse_macro_input, Data, DataEnum, DeriveInput, Ident, Variant};

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

fn generate_variant_context_match_arm(variant: &Variant, root_ident: &Ident) -> TokenStream2 {
    let variant_ident = &variant.ident;
    let children_ident = Ident::new("children", variant_ident.span());
    let field_fills = generate_field_fills(&variant.fields, root_ident, &children_ident);

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
                    if (children.len() < self.children().len()) {
                        return Err(UniplateError::NotEnoughChildren);
                    }

                    let mut #children_ident = children.clone();
                    Ok(#root_ident::#variant_ident(#(#field_fills,)*))
                })
            }
        }
    }
}

#[proc_macro_derive(Uniplate)]
pub fn derive(macro_input: TokenStream) -> TokenStream {
    let input = parse_macro_input!(macro_input as DeriveInput);
    let root_ident = &input.ident;
    let data = &input.data;

    let children_impl: TokenStream2 = match data {
        Data::Struct(_) => unimplemented!("Structs currently not supported"), // ToDo support structs
        Data::Union(_) => unimplemented!("Unions currently not supported"), // ToDo support unions
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

    let output = quote! {
        use uniplate::uniplate::UniplateError;

        impl Uniplate for #root_ident {
            fn uniplate(&self) -> (Vec<#root_ident>, Box<dyn Fn(Vec<#root_ident>) -> Result<#root_ident, UniplateError> + '_>) {
                let context: Box<dyn Fn(Vec<#root_ident>) -> Result<#root_ident, UniplateError>> = #context_impl;

                let children: Vec<#root_ident> = #children_impl;

                (children, context)
            }
        }
    };

    // println!("Final macro output:\n{}", output.to_string());

    output.into()
}
