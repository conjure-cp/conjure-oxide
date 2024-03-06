use proc_macro::{self, TokenStream};
use proc_macro2::{TokenStream as TokenStream2};
use quote::{quote, ToTokens};
use syn::{parse_macro_input, DeriveInput, Data, DataEnum, Fields, Ident};

#[proc_macro_derive(Uniplate)]
pub fn derive(macro_input: TokenStream) -> TokenStream {
    let input = parse_macro_input!(macro_input as DeriveInput);
    let enum_ident = &input.ident;
    let data = &input.data;

    let children_impl: TokenStream2 = match data {
        Data::Struct(_) => {unimplemented!("Structs currently not supported")}
        Data::Union(_) => {unimplemented!("Unions currently not supported")}
        Data::Enum(DataEnum {variants, ..}) => {
            let match_arms: Vec<TokenStream2> = variants.iter().map(|variant| {
                let variant_ident = &variant.ident;

                let field_names: Vec<Ident> = match &variant.fields {
                    Fields::Unit => {todo!()}
                    Fields::Named(_) => {todo!()}
                    Fields::Unnamed(fields) => {
                        fields.unnamed.iter().enumerate().map(|(idx, field)| {
                            let field_name = &format!("field_{}", idx);
                            Ident::new(field_name, enum_ident.span())
                        }).collect::<Vec<Ident>>()
                    }
                };

                quote! {
                    #enum_ident::#variant_ident(#(#field_names)*) => {

                    }
                }
            }).collect::<Vec<_>>();

            quote! {
                match self {
                    #(#match_arms)*
                }
            }
        }
    };

    let output = quote! {
        impl Uniplate for #enum_ident {
            fn uniplate(&self) -> (Vec<#enum_ident>, Box<dyn Fn(Vec<#enum_ident>) -> #enum_ident +'_>) {
                let context: Box<dyn Fn(Vec<#enum_ident>) -> #enum_ident> = match self {
                    _ => Box::new(|Vec<#enum_ident>| #enum_ident::A(0))
                };

                let children: Vec<#enum_ident> = #children_impl;

                (children, context)
            }
        }
    };
    output.into()
}

/*
let context: Box<dyn Fn(Vec<AST>) -> AST> = match self {
//!             AST::Int(i) =>    Box::new(|_| AST::Int(*i)),
//!             AST::Add(_, _) => Box::new(|exprs: Vec<AST>| AST::Add(Box::new(exprs[0].clone()),Box::new(exprs[1].clone()))),
//!             AST::Sub(_, _) => Box::new(|exprs: Vec<AST>| AST::Sub(Box::new(exprs[0].clone()),Box::new(exprs[1].clone()))),
//!             AST::Div(_, _) => Box::new(|exprs: Vec<AST>| AST::Div(Box::new(exprs[0].clone()),Box::new(exprs[1].clone()))),
//!             AST::Mul(_, _) => Box::new(|exprs: Vec<AST>| AST::Mul(Box::new(exprs[0].clone()),Box::new(exprs[1].clone())))
//!         };
//!
//!         let children: Vec<AST> = match self {
//!             AST::Add(a,b) => vec![*a.clone(),*b.clone()],
//!             AST::Sub(a,b) => vec![*a.clone(),*b.clone()],
//!             AST::Div(a,b) => vec![*a.clone(),*b.clone()],
//!             AST::Mul(a,b) => vec![*a.clone(),*b.clone()],
//!             _ => vec![]
//!         };
 */