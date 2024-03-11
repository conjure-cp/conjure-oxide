use crate::utils::parse::{check_field_type, parse_field_type, UniplateField};
use proc_macro2::{Ident, Literal, TokenStream as TokenStream2};
use quote::{quote, ToTokens};
use syn::spanned::Spanned;
use syn::{Field, Fields};

/// Generate the code to fill a field in a variant
fn get_fill(
    ft: &UniplateField,
    exprs_ident: &Ident,
    field_ident: &TokenStream2,
    root_ident: &Ident,
) -> TokenStream2 {
    if check_field_type(ft, root_ident) { // If the field or at least one of its children is a type we want to fill
        match ft {
            UniplateField::Identifier(_) => {
                return quote! {
                    #exprs_ident.remove(0) // If it is an identifier, take the next child from the list
                }
            }
            UniplateField::Box(_, subfield) => {
                let sf = subfield.as_ref();
                let sf_fill = get_fill(sf, exprs_ident, field_ident, root_ident);
                return quote! {
                    Box::new(#sf_fill) // If it is a box, generate the fill for the inner type and box it
                };
            }
            UniplateField::Vector(_, subfield) => {
                let sf = subfield.as_ref();
                let sf_fill = get_fill(sf, exprs_ident, field_ident, root_ident);
                return quote! { // The size is not known at compile time, so generate a loop to fill the vector (using the appropriate fill for the inner type)
                    {
                        let mut elems: Vec<_> = Vec::new();
                        for i in 0..#field_ident.len() { // The length of vectors must not change, so we can use the length of the field to determine how many children to take
                            elems.push(#sf_fill)
                        }
                        elems
                    }
                };
            }
            UniplateField::Tuple(_, sfs) => {
                let mut sf_fills: Vec<TokenStream2> = Vec::new();

                for (i, sf) in sfs.iter().enumerate() { // Recursively generate the fill for each field in the tuple
                    let i_literal = Literal::usize_unsuffixed(i);
                    let sf_ident = quote! {
                        #field_ident.#i_literal
                    };
                    sf_fills.push(get_fill(sf, exprs_ident, &sf_ident, root_ident));
                }

                return quote! {
                    (#(#sf_fills,)*) // Wrap the fills in a tuple
                };
            }
            UniplateField::Array(_, _, _) => {
                unimplemented!("Arrays not currently supported") // ToDo support arrays
            }
            UniplateField::Unknown(_) => {}
        }
    }

    quote! {
        #field_ident.clone() // If the field is not a type we want to fill, just keep it
    }
}

/// Generate the code to clone a field in a variant
fn get_clone(
    ft: &UniplateField,
    field_ident: TokenStream2,
    root_ident: &Ident,
) -> Option<TokenStream2> {
    if check_field_type(ft, root_ident) { // If the field or at least one of its children is a type we want to clone
        match ft {
            UniplateField::Identifier(_) => {
                return Some(quote! {
                    vec![#field_ident.clone()] // If it is an identifier, clone it. We still need to wrap it in a vec to use .flatten() on the final list.
                });
            }
            UniplateField::Box(_, inner) => {
                let sf = inner.as_ref();
                let box_clone = quote! { // Generate the prefix for getting the inner type out of the box
                    #field_ident.as_ref().clone()
                };
                return get_clone(sf, box_clone, root_ident); // Then generate the clone for the inner type
            }
            UniplateField::Vector(_, inner) => {
                let sf = inner.as_ref();

                let sf_ident = Ident::new("sf", sf.span()).into_token_stream(); // Identity for the subfields
                let sf_clone = get_clone(sf, sf_ident, root_ident); // Clone for the subfields

                return Some(quote! {
                    #field_ident.iter().flat_map(|sf| #sf_clone).collect::<Vec<_>>() // If it is a vector, generate the clone for the inner type and flatten the list
                });
            }
            UniplateField::Tuple(_, sfs) => {
                let mut sf_clones: Vec<TokenStream2> = Vec::new();

                for (i, sf) in sfs.iter().enumerate() { // Recursively generate the clone for each field in the tuple
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

                return Some(quote! { // Clone the subfields into a vec and flatten
                    vec![#(#sf_clones,)*].iter().flatten().cloned().collect::<Vec<_>>()
                });
            }
            UniplateField::Array(_, _, _) => { // ToDo support arrays
                unimplemented!("Arrays not currently supported")
            }
            UniplateField::Unknown(_) => {} // Ignore unknown types
        }
    }

    None // If the field is not a type we want to clone, return None
}

/// Helper function to get the name of a field - if it has no name, use `field{idx}`
fn get_field_name(field: &Field, idx: usize) -> String {
    match &field.ident {
        None => format!("field{}", idx),
        Some(ident) => ident.to_string(),
    }
}

/// Generate the code to match the fields of a variant
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

/// Generate the code to clone the fields of a variant
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

/// Generate the code to fill the fields of a variant
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
