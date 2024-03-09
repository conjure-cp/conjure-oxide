use proc_macro::{self, TokenStream};
use proc_macro2::{Span, TokenStream as TokenStream2};
use quote::{quote, ToTokens};
use syn::{parse_macro_input, DeriveInput, Data, DataEnum, Fields, Ident, Type, PathArguments, GenericArgument, Expr};
use syn::spanned::Spanned;
use syn::token::Underscore;
use crate::UniplateField::Unknown;

enum ParseTypeArgumentError {
    NoTypeArguments,
    EmptyTypeArguments,
    MultipleTypeArguments,
    TypeArgumentNotAType,
    TypeArgumentValueNotPath,
    TypeArgumentEmptyPath,
}

enum UniplateField {
    Identifier(Ident),
    Box(Span, Box<UniplateField>),
    Vector(Span, Box<UniplateField>),
    Tuple(Span, Vec<UniplateField>),
    Array(Span, Box<UniplateField>, Expr),
    Unknown(Span),
}

fn get_span(field: &UniplateField) -> Span {
    match field {
        UniplateField::Identifier(idnt) => idnt.span(),
        UniplateField::Box(spn, _) => *spn,
        UniplateField::Vector(spn, _) => *spn,
        UniplateField::Tuple(spn, _) => *spn,
        UniplateField::Array(spn, _, _) => *spn,
        UniplateField::Unknown(spn) => *spn
    }
}

fn parse_type_argument(seg_args: &PathArguments) -> Result<&Ident, ParseTypeArgumentError> {
    match seg_args {
        PathArguments::AngleBracketed(type_args) => {
            if type_args.args.len() > 1 {
                return Err(ParseTypeArgumentError::MultipleTypeArguments);
            }

            match type_args.args.last() {
                None => Err(ParseTypeArgumentError::EmptyTypeArguments),
                Some(arg) => {
                    match arg {
                        GenericArgument::Type(tp) => {
                            match tp {
                                Type::Path(pth) => {
                                    match pth.path.segments.last() {
                                        Some(seg) => Ok(&seg.ident),
                                        None => Err(ParseTypeArgumentError::TypeArgumentEmptyPath)
                                    }
                                }
                                _ => Err(ParseTypeArgumentError::TypeArgumentValueNotPath)
                            }
                        }
                        _ => Err(ParseTypeArgumentError::TypeArgumentNotAType)
                    }
                }
            }
        }
        _ => Err(ParseTypeArgumentError::NoTypeArguments)
    }
}

fn parse_field_type(field_type: &Type) -> UniplateField {
    match field_type {
        Type::Path(path) => match path.path.segments.last() {
            None => Unknown(path.span()),
            Some(seg) => {
                let ident = &seg.ident;
                let span = ident.span();
                let args = &seg.arguments;

                let box_ident = &Ident::new("Box", span);
                let vec_ident = &Ident::new("Vec", span);

                if ident.eq(box_ident) {
                    match parse_type_argument(args) {
                        Ok(idnt) => UniplateField::Box(path.span(), Box::new(UniplateField::Identifier(idnt.clone()))),
                        Err(_) => Unknown(ident.span())
                    }
                } else if ident.eq(vec_ident) {
                    match parse_type_argument(args) {
                        Ok(idnt) => UniplateField::Vector(path.span(), Box::new(UniplateField::Identifier(idnt.clone()))),
                        Err(_) => Unknown(ident.span())
                    }
                } else {
                    UniplateField::Identifier(ident.clone())
                }
            }
        }
        Type::Tuple(tpl) => UniplateField::Tuple(tpl.span(), tpl.elems.iter().map(parse_field_type).collect()),
        Type::Array(arr) => UniplateField::Array(arr.span(), Box::new(parse_field_type(arr.elem.as_ref())), arr.len.clone()),
        _ => Unknown(field_type.span()) // ToDo discuss - Can we support any of: BareFn, Group, ImplTrait, Infer, Macro, Never, Paren, Ptr, Reference, TraitObject, Verbatim
    }
}

fn check_field_type(ft: &UniplateField, root_ident: &Ident) -> bool {
    match ft {
        UniplateField::Identifier(ident) => ident.eq(root_ident),
        UniplateField::Box(_, subfield) => check_field_type(subfield.as_ref(), root_ident),
        UniplateField::Vector(_, subfield) => check_field_type(subfield.as_ref(), root_ident),
        UniplateField::Tuple(_, subfields) => {
            for sft in subfields {
                if check_field_type(sft, root_ident) {
                    return true;
                }
            }
            false
        }
        UniplateField::Array(_, arr_type, _) => check_field_type(arr_type.as_ref(), root_ident),
        UniplateField::Unknown(_) => false
    }
}

fn get_ident_and_clone(ft: &UniplateField, field_name: String, root_ident: &Ident) -> (TokenStream2, Vec<TokenStream2>) {
    let span = get_span(ft);

    match ft {
        UniplateField::Identifier(_) => {
            if check_field_type(ft, root_ident) {
                let ident = Ident::new(&format!("{}", field_name), span).into_token_stream();
                let clone = quote! {
                    #ident.clone()
                };
                return (ident, vec![clone])
            }
        }
        UniplateField::Vector(_, sft) => {
            if check_field_type(sft, root_ident) {
                let ident = Ident::new(&format!("{}_vec", field_name), span).into_token_stream();
                let clone = quote! {
                    *#ident.clone()
                };
                return (ident, vec![clone])
            }
        }
        UniplateField::Box(_, sft) => {
            if check_field_type(sft, root_ident) {
                let ident = Ident::new(&format!("{}_box", field_name), span).into_token_stream();
                let clone = quote! {
                    *#ident.as_ref().clone()
                };
                return (ident, vec![clone])
            }
        }
        UniplateField::Tuple(_, subfields) => {
            let mut subfield_idents: Vec<TokenStream2> = Vec::new();
            let mut subfield_clones: Vec<TokenStream2> = Vec::new();

            for (i, sft) in subfields.iter().enumerate() {
                let sfname = format!("{}_tpl_{}", field_name, i);
                let (sfident, sfclones) = get_ident_and_clone(sft, sfname, root_ident);
                subfield_idents.push(sfident);
                subfield_clones.extend(sfclones);
            }

            let ident = quote! {
                (#(#subfield_idents,)*)
            };

            return (ident, subfield_clones)
        },
        UniplateField::Array(_, of_type, _) => {
            if check_field_type(of_type, root_ident) {
                let ident = Ident::new(&format!("{}_arr", field_name), span).into_token_stream();
                let clone = quote! {
                    #ident.clone()
                };

                return (ident, vec![clone])
            }
        }
        UniplateField::Unknown(_) => {}
    }

    (Underscore(span).into_token_stream(), vec![])
}

fn field_idents_and_clones(fields: &Fields, root_ident: &Ident) -> Vec<(TokenStream2, TokenStream2)> {
    return fields.iter().enumerate().map(|(idx, field)| {
        let field_name = match &field.ident {
            None => format!("field{}", idx),
            Some(ident) => ident.to_string()
        };
        let field_type = parse_field_type(&field.ty);

        let (ident, clones) = get_ident_and_clone(&field_type, field_name, root_ident);
        let clone = quote! {
                vec![#(#clones,)*]
            };

        (ident, clone)
    }).collect()
}

#[proc_macro_derive(Uniplate)]
pub fn derive(macro_input: TokenStream) -> TokenStream {
    let input = parse_macro_input!(macro_input as DeriveInput);
    let root_ident = &input.ident;
    let data = &input.data;

    let children_impl: TokenStream2 = match data {
        Data::Struct(_) => { unimplemented!("Structs currently not supported") }
        Data::Union(_) => { unimplemented!("Unions currently not supported") }
        Data::Enum(DataEnum { variants, .. }) => {
            let match_arms: Vec<TokenStream2> = variants.iter().map(|variant| {
                let idents_and_clones = field_idents_and_clones(&variant.fields, root_ident);
                let field_idents: Vec<&TokenStream2> = idents_and_clones.iter().map(|tpl| &tpl.0).collect();
                let field_clones: Vec<&TokenStream2> = idents_and_clones.iter().map(|tpl| &tpl.1).collect();
                let variant_ident = &variant.ident;

                let match_pattern = if field_idents.is_empty() {
                    quote! {
                        #root_ident::#variant_ident
                    }
                } else {
                    quote! {
                        #root_ident::#variant_ident(#(#field_idents,)*)
                    }
                };

                let mach_arm = quote! {
                     #match_pattern => {
                        vec![#(#field_clones,)*].iter().flatten().collect()
                    }
                };

                println!("Generated match arm: {}", mach_arm.to_string());

                mach_arm
            }).collect::<Vec<_>>();

            let match_statement = quote! {
                match self {
                    #(#match_arms)*
                }
            };

            println!("Generated match statement for {}: \n{}", root_ident.to_string(), match_statement.to_string());

            match_statement
        }
    };

    let output = quote! {
        impl Uniplate for #root_ident {
            fn uniplate(&self) -> (Vec<#root_ident>, Box<dyn Fn(Vec<#root_ident>) -> #root_ident +'_>) {
                let context: Box<dyn Fn(Vec<#root_ident>) -> #root_ident> = match self {
                    _ => Box::new(|children| #root_ident::A(0))
                };

                let children: Vec<#root_ident> = #children_impl;

                (children, context)
            }
        }
    };

    println!("Final macro output:\n{}", output.to_string());

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