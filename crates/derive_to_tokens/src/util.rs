//! Helpers that don't fit anywhere else.

use deluxe::ExtractAttributes;
use quote::ToTokens;
use std::collections::HashSet;
use itertools::Itertools;
use syn::punctuated::Punctuated;
use syn::{parse_quote, GenericArgument, PathArguments};
use syn::{Data, DeriveInput, Error, Field, Fields, Result, Token, Type};
use syn::{TypeParamBound, WhereClause, WherePredicate};

/// Generates a `where` clause with the specified bounds applied to all field types.
pub fn add_bounds(
    input: DeriveInput,
    where_clause: Option<&WhereClause>,
    bounds: Punctuated<TypeParamBound, Token![+]>,
) -> Result<WhereClause> {
    let unique_types: HashSet<_> = match input.data {
        Data::Union(_) => return Err(Error::new_spanned(input, "unions are not supported")),
        Data::Struct(data) => match data.fields {
            Fields::Unit => HashSet::new(),
            Fields::Named(fields) => fields
                .named
                .into_iter()
                .flat_map(|f| field_leaf_types(f))
                .collect::<HashSet<_>>(),
                //.filter_map(|f| field_type(f).transpose())
                //.collect::<Result<_>>()?,
            Fields::Unnamed(fields) => fields
                .unnamed
                .into_iter()
                .flat_map(|f| field_leaf_types(f))
                .collect::<HashSet<_>>(),
                // .filter_map(|f| field_type(f).transpose())
                // .collect::<Result<_>>()?,
        },
        Data::Enum(data) => data
            .variants
            .into_iter()
            .flat_map(|v| match v.fields {
                Fields::Unit => Vec::new(),
                Fields::Named(fields) => fields
                    .named
                    .into_iter()
                    .flat_map(|f| field_leaf_types(f))
                    .collect::<Vec<_>>(),
                    // .filter_map(|f| field_type(f).transpose())
                    // .collect::<Vec<_>>(),
                Fields::Unnamed(fields) => fields
                    .unnamed
                    .into_iter()
                    .flat_map(|f| field_leaf_types(f))
                    .collect::<Vec<_>>(),
                    // .filter_map(|f| field_type(f).transpose())
                    // .collect::<Vec<_>>(),
            })
            .collect::<HashSet<_>>(),
            // .collect::<Result<_>>()?,
    };

    let mut where_clause = where_clause.cloned().unwrap_or_else(|| WhereClause {
        where_token: Default::default(),
        predicates: Default::default(),
    });

    where_clause
        .predicates
        .extend(unique_types.iter().map(|ty| -> WherePredicate {
            parse_quote! {
                #ty: #bounds
            }
        }));

    Ok(where_clause)
}

/// Return the type of the field if it isn't marked
/// with the `#[to_tokens(recursive)]` attribute.
fn field_type(mut field: Field) -> Result<Option<Type>> {
    let attrs: Attrs = deluxe::extract_attributes(&mut field)?;
    let ty = if attrs.recursive {
        None
    } else {
        Some(field.ty)
    };

    Ok(ty)
}

/// Helper type for parsing the meta attributes of the
/// type for which `Parse` and `ToTokens` are being `#[derive]`d.
#[derive(Clone, Default, Debug, ExtractAttributes)]
#[deluxe(attributes(to_tokens))]
pub struct Attrs {
    /// Indicates that the field participates in (possibly mutual) recursion
    /// at the type level, e.g. a parent-child relationship within the same
    /// struct/enum. The type of such fields will be omitted from the `where`
    /// clause in the derived implementations, becuse the corresponding
    /// constraints can't be satisfied, and the code compiles without them.
    ///
    /// Hopefully, this can be removed in the future once Chalk lands;
    /// see [this issue](https://github.com/rust-lang/rust/issues/48214)
    #[deluxe(default = false)]
    pub recursive: bool,
}

pub enum FieldWrapper {
    Box(Type),
    Vec(Type),
    Option(Type),
    Tuple(Vec<Type>),
}

pub fn field_leaf_types(mut field: Field) -> Vec<Type> {
    let attrs: Attrs = deluxe::extract_attributes(&mut field).unwrap();
    if attrs.recursive {
        return vec![];
    }
    field_leaf_types_impl(&field.ty)
}
pub fn field_leaf_types_impl(ty: &Type) -> Vec<Type> {
    match field_wrapper(ty) {
        Some(fw) => match &fw {
            FieldWrapper::Box(inner) | FieldWrapper::Vec(inner) | FieldWrapper::Option(inner) => field_leaf_types_impl(inner),
            FieldWrapper::Tuple(inner) => inner.iter().flat_map(field_leaf_types_impl).collect(),
        }
        None => vec![ty.clone()],
    }
}

pub fn field_wrapper(ty: &Type) -> Option<FieldWrapper> {
    // println!("Field: {}", ty.into_token_stream().to_string());
    // println!("AST: {:#?}", ty);
    match ty {
        Type::Path(path) => {
            let last = path.path.segments.last().unwrap();
            let ident = last.ident.to_string();
            let inners = match last.arguments {
                PathArguments::AngleBracketed(ref args) => {
                    args.args.iter().filter_map(|a| {
                        match a {
                            GenericArgument::Type(inner) => Some(inner.clone()),
                            _ => {
                                println!("Couldn't parse generic type argument: {:#?}", a);
                                None
                            }
                        }
                    }).collect_vec()
                }
                _ => {
                    // println!("Invalid type arguments for: {:#?}", ty);
                    vec![]
                }
            };
            match ident.as_str() {
                "Option" | "Box" | "Vec" => {
                    if inners.len() != 1 {
                        // println!("Invalid type arguments for: {:#?}", ty);
                        // println!("Expected 1, got: {:#?}", inners);
                        panic!("Expected 1, got: {:#?}", inners);
                    }
                    match ident.as_str() {
                        "Option" => Some(FieldWrapper::Option(inners[0].clone())),
                        "Box" => Some(FieldWrapper::Box(inners[0].clone())),
                        "Vec" => Some(FieldWrapper::Vec(inners[0].clone())),
                        _ => unreachable!(),
                    }
                }
                _ => None,
            }
        }
        Type::Tuple(inner) => {
            let mut tuple = Vec::new();
            for ty in inner.elems.iter() {
                tuple.push(ty.clone());
            }
            Some(FieldWrapper::Tuple(tuple))
        }
        _ => None,
    }
}
