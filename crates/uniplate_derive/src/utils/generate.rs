use crate::utils::parse::{check_field_type, parse_field_type, UniplateField};
use proc_macro2::{Ident, Literal, TokenStream as TokenStream2};
use quote::{quote, ToTokens};
use syn::spanned::Spanned;
use syn::{Field, Fields};

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
            UniplateField::Array(_, _, _) => {
                unimplemented!("Arrays not currently supported") // ToDo support arrays
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

                let sf_ident = Ident::new("sf", sf.span()).into_token_stream();
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
            UniplateField::Unknown(_) => {}
        }
    }

    None
}

fn get_field_name(field: &Field, idx: usize) -> String {
    match &field.ident {
        None => format!("field{}", idx),
        Some(ident) => ident.to_string(),
    }
}

pub fn generate_field_idents(fields: &Fields) -> Vec<TokenStream2> {
    return fields
        .iter()
        .enumerate()
        .map(|(idx, field)| {
            let field_name = get_field_name(field, idx);
            Ident::new(&field_name, field.ident.span()).into_token_stream()
        })
        .collect();
}

pub fn generate_field_clones(fields: &Fields, root_ident: &Ident) -> Vec<TokenStream2> {
    return fields
        .iter()
        .enumerate()
        .filter_map(|(idx, field)| {
            let field_name = get_field_name(field, idx);
            let field_type = parse_field_type(&field.ty);
            let field_ident = Ident::new(&field_name, field.ident.span()).into_token_stream();

            get_clone(&field_type, field_ident, root_ident)
        })
        .collect();
}

pub fn generate_field_fills(
    fields: &Fields,
    root_ident: &Ident,
    exprs_ident: &Ident,
) -> Vec<TokenStream2> {
    return fields
        .iter()
        .enumerate()
        .map(|(idx, field)| {
            let field_name = get_field_name(field, idx);
            let field_type = parse_field_type(&field.ty);
            let field_ident = Ident::new(&field_name, field.ident.span()).into_token_stream();

            get_fill(&field_type, exprs_ident, &field_ident, root_ident)
        })
        .collect();
}
