use crate::UniplateField::Unknown;
use proc_macro::{self, TokenStream};
use proc_macro2::{Literal, Span, TokenStream as TokenStream2};
use quote::{quote, ToTokens};
use syn::spanned::Spanned;
use syn::{
    parse_macro_input, Data, DataEnum, DeriveInput, Expr, Field, Fields, GenericArgument, Ident,
    PathArguments, PathSegment, Type, Variant,
};

enum ParseTypeArgumentError {
    NoTypeArguments,
    EmptyTypeArguments,
    MultipleTypeArguments,
    TypeArgumentNotAType,
    TypeArgumentValueNotPath,
    TypeArgumentEmptyPath,
}

#[derive(Debug)]
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
        UniplateField::Unknown(spn) => *spn,
    }
}

fn parse_type_argument(seg_args: &PathArguments) -> Result<&PathSegment, ParseTypeArgumentError> {
    match seg_args {
        PathArguments::AngleBracketed(type_args) => {
            if type_args.args.len() > 1 {
                return Err(ParseTypeArgumentError::MultipleTypeArguments);
            }

            match type_args.args.last() {
                None => Err(ParseTypeArgumentError::EmptyTypeArguments),
                Some(arg) => match arg {
                    GenericArgument::Type(tp) => match tp {
                        Type::Path(pth) => match pth.path.segments.last() {
                            Some(seg) => Ok(seg),
                            None => Err(ParseTypeArgumentError::TypeArgumentEmptyPath),
                        },
                        _ => Err(ParseTypeArgumentError::TypeArgumentValueNotPath),
                    },
                    _ => Err(ParseTypeArgumentError::TypeArgumentNotAType),
                },
            }
        }
        _ => Err(ParseTypeArgumentError::NoTypeArguments),
    }
}

fn parse_field_type(field_type: &Type) -> UniplateField {
    fn parse_type(seg: &PathSegment) -> UniplateField {
        let ident = &seg.ident;
        let span = ident.span();
        let args = &seg.arguments;

        let box_ident = &Ident::new("Box", span);
        let vec_ident = &Ident::new("Vec", span);

        if ident.eq(box_ident) {
            match parse_type_argument(args) {
                Ok(inner_seg) => UniplateField::Box(seg.span(), Box::new(parse_type(inner_seg))),
                Err(_) => Unknown(ident.span()),
            }
        } else if ident.eq(vec_ident) {
            match parse_type_argument(args) {
                Ok(inner_seg) => UniplateField::Vector(seg.span(), Box::new(parse_type(inner_seg))),
                Err(_) => Unknown(ident.span()),
            }
        } else {
            UniplateField::Identifier(ident.clone())
        }
    }

    match field_type {
        Type::Path(path) => match path.path.segments.last() {
            None => Unknown(path.span()),
            Some(seg) => parse_type(seg),
        },
        Type::Tuple(tpl) => {
            UniplateField::Tuple(tpl.span(), tpl.elems.iter().map(parse_field_type).collect())
        }
        Type::Array(arr) => UniplateField::Array(
            arr.span(),
            Box::new(parse_field_type(arr.elem.as_ref())),
            arr.len.clone(),
        ),
        _ => Unknown(field_type.span()), // ToDo discuss - Can we support any of: BareFn, Group, ImplTrait, Infer, Macro, Never, Paren, Ptr, Reference, TraitObject, Verbatim
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
        UniplateField::Unknown(_) => false,
    }
}

fn get_fill(
    ft: &UniplateField,
    exprs_ident: &Ident,
    field_ident: &TokenStream2,
    root_ident: &Ident,
) -> TokenStream2 {
    if check_field_type(ft, root_ident) {
        match ft {
            UniplateField::Identifier(_) => {
                return quote! {
                    #exprs_ident.remove(0)
                }
            }
            UniplateField::Box(_, subfield) => {
                let sf = subfield.as_ref();
                let sf_fill = get_fill(sf, exprs_ident, field_ident, root_ident);
                return quote! {
                    Box::new(#sf_fill)
                };
            }
            UniplateField::Vector(_, subfield) => {
                let sf = subfield.as_ref();
                let sf_fill = get_fill(sf, exprs_ident, field_ident, root_ident);
                return quote! {
                    {
                        let mut elems: Vec<_> = Vec::new();
                        for i in 0..#field_ident.len() {
                            elems.push(#sf_fill)
                        }
                        elems
                    }
                };
            }
            UniplateField::Tuple(_, sfs) => {
                let mut sf_fills: Vec<TokenStream2> = Vec::new();

                for (i, sf) in sfs.iter().enumerate() {
                    let i_literal = Literal::usize_unsuffixed(i);
                    let sf_ident = quote! {
                        #field_ident.#i_literal
                    };
                    sf_fills.push(get_fill(sf, exprs_ident, &sf_ident, root_ident));
                }

                return quote! {
                    (#(#sf_fills,)*)
                };
            }
            UniplateField::Array(_, arr_type, _) => {
                unimplemented!("Arrays not currently supported")
            }
            UniplateField::Unknown(_) => {}
        }
    }

    quote! {
        #field_ident.clone()
    }
}

fn get_clone(
    ft: &UniplateField,
    field_ident: TokenStream2,
    root_ident: &Ident,
) -> Option<TokenStream2> {
    if check_field_type(ft, root_ident) {
        match ft {
            UniplateField::Identifier(_) => {
                return Some(quote! {
                    vec![#field_ident.clone()]
                });
            }
            UniplateField::Box(_, inner) => {
                let sf = inner.as_ref();
                let box_clone = quote! {
                    #field_ident.as_ref().clone()
                };
                return get_clone(sf, box_clone, root_ident);
            }
            UniplateField::Vector(_, inner) => {
                let sf = inner.as_ref();

                let sf_ident = Ident::new("sf", get_span(sf)).into_token_stream();
                let sf_clone = get_clone(sf, sf_ident, root_ident);

                return Some(quote! {
                    #field_ident.iter().flat_map(|sf| #sf_clone).collect::<Vec<_>>()
                });
            }
            UniplateField::Tuple(_, sfs) => {
                let mut sf_clones: Vec<TokenStream2> = Vec::new();

                for (i, sf) in sfs.iter().enumerate() {
                    let i_literal = Literal::usize_unsuffixed(i);
                    let sf_ident = quote! {
                        #field_ident.#i_literal
                    };
                    let sf_clone = get_clone(sf, sf_ident, root_ident);
                    match sf_clone {
                        None => {}
                        Some(sfc) => sf_clones.push(sfc),
                    }
                }

                return Some(quote! {
                    vec![#(#sf_clones,)*].iter().flatten().cloned().collect::<Vec<_>>()
                });
            }
            UniplateField::Array(_, _, _) => {
                unimplemented!("Arrays not currently supported")
            }
            Unknown(_) => {}
        }
    }

    None
}

fn make_field_name(field: &Field, idx: usize) -> String {
    match &field.ident {
        None => format!("field{}", idx),
        Some(ident) => ident.to_string(),
    }
}

fn field_idents(fields: &Fields) -> Vec<TokenStream2> {
    return fields
        .iter()
        .enumerate()
        .map(|(idx, field)| {
            let field_name = make_field_name(field, idx);
            Ident::new(&field_name, field.ident.span()).into_token_stream()
        })
        .collect();
}

fn field_clones(fields: &Fields, root_ident: &Ident) -> Vec<TokenStream2> {
    return fields
        .iter()
        .enumerate()
        .filter_map(|(idx, field)| {
            let field_name = make_field_name(field, idx);
            let field_type = parse_field_type(&field.ty);
            let field_ident = Ident::new(&field_name, field.ident.span()).into_token_stream();

            get_clone(&field_type, field_ident, root_ident)
        })
        .collect();
}

fn field_fills(fields: &Fields, root_ident: &Ident, exprs_ident: &Ident) -> Vec<TokenStream2> {
    return fields
        .iter()
        .enumerate()
        .map(|(idx, field)| {
            let field_name = make_field_name(field, idx);
            let field_type = parse_field_type(&field.ty);
            let field_ident = Ident::new(&field_name, field.ident.span()).into_token_stream();

            get_fill(&field_type, exprs_ident, &field_ident, root_ident)
        })
        .collect();
}

fn generate_match_pattern(variant: &Variant, root_ident: &Ident) -> TokenStream2 {
    let field_idents = field_idents(&variant.fields);
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
    let field_clones = field_clones(&variant.fields, root_ident);

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
    let field_fills = field_fills(&variant.fields, root_ident, &children_ident);

    let match_pattern = generate_match_pattern(variant, root_ident);

    if field_fills.is_empty() {
        quote! {
            #match_pattern => {
                Box::new(|_| root_ident::#variant_ident())
            }
        }
    } else {
        quote! {
            #match_pattern => {
                Box::new(|children| {
                    let mut #children_ident = children.clone();
                    #root_ident::#variant_ident(#(#field_fills,)*)
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
        Data::Struct(_) => unimplemented!("Structs currently not supported"),
        Data::Union(_) => unimplemented!("Unions currently not supported"),
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
        impl Uniplate for #root_ident {
            fn uniplate(&self) -> (Vec<#root_ident>, Box<dyn Fn(Vec<#root_ident>) -> #root_ident +'_>) {
                let context: Box<dyn Fn(Vec<#root_ident>) -> #root_ident> = #context_impl;

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
    AST::Int(i) =>    Box::new(|_| AST::Int(*i)),
    AST::Add(_, _) => Box::new(|exprs: Vec<AST>| AST::Add(Box::new(exprs[0].clone()),Box::new(exprs[1].clone()))),
    AST::Sub(_, _) => Box::new(|exprs: Vec<AST>| AST::Sub(Box::new(exprs[0].clone()),Box::new(exprs[1].clone()))),
    AST::Div(_, _) => Box::new(|exprs: Vec<AST>| AST::Div(Box::new(exprs[0].clone()),Box::new(exprs[1].clone()))),
    AST::Mul(_, _) => Box::new(|exprs: Vec<AST>| AST::Mul(Box::new(exprs[0].clone()),Box::new(exprs[1].clone())))
};

let children: Vec<AST> = match self {
    AST::Add(a,b) => vec![*a.clone(),*b.clone()],
    AST::Sub(a,b) => vec![*a.clone(),*b.clone()],
    AST::Div(a,b) => vec![*a.clone(),*b.clone()],
    AST::Mul(a,b) => vec![*a.clone(),*b.clone()],
    _ => vec![]
};
 */
