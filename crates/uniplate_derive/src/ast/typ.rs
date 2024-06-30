use crate::prelude::*;
use itertools::Itertools;
use lazy_static::lazy_static;
use quote::TokenStreamExt;

/// All valid field smart pointer  - e.g Box, Vec, ...
#[derive(Clone, Debug)]
pub enum BoxType {
    Box,
}

impl ToTokens for BoxType {
    fn to_tokens(&self, tokens: &mut TokenStream2) {
        match self {
            BoxType::Box => {
                tokens.append_all(quote! {Box});
            }
        }
    }
}

#[derive(Clone, Debug)]
pub enum Type {
    BoxedPlateable(BoxedPlateableType),
    Plateable(PlateableType),
    Unplateable,
}

impl Type {
    pub fn base_typ(&self) -> Option<syn::Path> {
        match self {
            Type::BoxedPlateable(x) => Some(x.base_typ()),
            Type::Plateable(x) => Some(x.base_typ()),
            Type::Unplateable => None,
        }
    }
}

impl ToTokens for Type {
    fn to_tokens(&self, tokens: &mut TokenStream2) {
        match self {
            Type::BoxedPlateable(x) => x.to_tokens(tokens),
            Type::Plateable(x) => x.to_tokens(tokens),
            Type::Unplateable => (),
        }
    }
}

pub trait HasBaseType {
    fn base_typ(&self) -> syn::Path;
}

lazy_static! {
    static ref BOX_PREFIXES: Vec<&'static str> =
        vec!("::std::boxed::Box", "std::boxed::Box", "Box");
}

impl Parse for Type {
    fn parse(input: syn::parse::ParseStream) -> syn::Result<Self> {
        let typ: syn::Type = input.parse()?;
        let syn::Type::Path(typ) = typ else {
            return Ok(Type::Unplateable);
        };

        let mut base_path = typ.path.clone();
        let mut wrapper_path: Option<syn::Path> = Some(typ.path);

        let mut box_type: Option<BoxType> = None;
        let mut any_args = false; // special case: if we find no args wrapper type should be empty.

        // Is the outermost type a box?
        let type_str: String = base_path
            .segments
            .iter()
            .map(|x| x.ident.to_string())
            .intersperse("::".to_owned())
            .collect();

        if BOX_PREFIXES.contains(&type_str.as_str()) {
            box_type = Some(BoxType::Box);
            let syn::PathArguments::AngleBracketed(args) =
                &base_path.segments.last().expect("").arguments.clone()
            else {
                panic!();
            };

            let syn::GenericArgument::Type(syn::Type::Path(typ2)) = args.args.first().expect("")
            else {
                return Err(syn::Error::new(
                    args.span(),
                    "Biplate: expected type argument here",
                ));
            };
            base_path = typ2.path.clone();

            any_args = false;
        }

        while let syn::PathArguments::AngleBracketed(args) =
            &base_path.segments.last().expect("").arguments.clone()
        {
            any_args = true;
            if args.args.len() != 1 {
                return Err(syn::Error::new(
                    args.span(),
                    format!(
                        "Biplate: expected one generic argument here, got {}",
                        args.args.len()
                    ),
                ));
            }

            let syn::GenericArgument::Type(syn::Type::Path(typ2)) = args.args.first().expect("")
            else {
                return Err(syn::Error::new(
                    args.span(),
                    "Biplate: expected type argument here",
                ));
            };

            base_path = typ2.path.clone();

            // Have we just found a box type?
            let type_str: String = base_path
                .segments
                .iter()
                .map(|x| x.ident.to_string())
                .intersperse("::".to_owned())
                .collect();

            let mut new_box_type: Option<BoxType> = None;
            if BOX_PREFIXES.contains(&type_str.as_str()) {
                new_box_type = Some(BoxType::Box);
            }

            // Have a Box<Box<T>> - I don't know how to handle this
            if new_box_type.is_some() && box_type.is_some() {
                return Err(syn::Error::new(
                    args.span(),
                    "Biplate: nested Box<> is not supported.",
                ));
            }

            if new_box_type.is_some() {
                box_type = new_box_type;

                wrapper_path = Some(base_path.clone());
                any_args = false;
            }
        }

        // ensure that we don't have parenthesised (...) type arguments.
        let args = base_path.segments.last().expect("").arguments.clone();
        let syn::PathArguments::None = args else {
            return Err(syn::Error::new(
                args.span(),
                "Biplate: expected no type arguments here.",
            ));
        };

        if !any_args {
            // if we have no arguments in our path, there is no wrapper path.
            wrapper_path = None;
        } else {
            wrapper_path
                .clone()
                .expect("")
                .segments
                .last_mut()
                .expect("")
                .arguments = syn::PathArguments::None;
        }

        let plateable_typ = PlateableType {
            wrapper_typ: wrapper_path,
            base_typ: base_path,
        };

        if let Some(box_type) = box_type {
            Ok(Type::BoxedPlateable(BoxedPlateableType {
                inner_typ: plateable_typ,
                box_typ: box_type,
            }))
        } else {
            Ok(Type::Plateable(plateable_typ))
        }
    }
}

/// A platable type inside a smart-pointer or cell.
///
/// Unlike most `PlateableType`s, the conversions from Box<T> to T are inlined in code generation
/// instead of using builtin implementations of Biplate.
///
/// This is to avoid unnecessary moving of stuff between stack and heap - instead, we just
/// dereference the smart pointer and pass that into Biplate<T>.
#[derive(Clone, Debug)]
pub struct BoxedPlateableType {
    /// The underlying type of the field.
    pub inner_typ: PlateableType,

    /// The wrapper type of the field.
    pub box_typ: BoxType,
}

impl ToTokens for BoxedPlateableType {
    fn to_tokens(&self, tokens: &mut TokenStream2) {
        let base_typ = self.inner_typ.clone();
        match self.box_typ {
            BoxType::Box => {
                tokens.append_all(quote! {Box<#base_typ>});
            }
        }
    }
}

impl HasBaseType for BoxedPlateableType {
    fn base_typ(&self) -> syn::Path {
        self.inner_typ.base_typ()
    }
}

/// A plateable type.
///
/// This struct splits a type into wrapper and base components.
/// Base types are used to determine what new instances of Biplate to derive.
/// Wrapper types are unwrapped through builtin impls of uniplate.
///
/// For example, Vec<Vec<MyTyp>>  has the base type MyTyp and the wrapper type Vec<Vec<.
/// This distinction between base and wrapper here means that we only derive Biplate for MyTyp, and
/// we use preexisting rules to unwrap the vectors surrounding it.unwrhandle
///
/// Boxed / smart pointer types are handled differently - see `BoxedPlatableType`.
#[derive(Clone, Debug)]
pub struct PlateableType {
    /// Container types of the field
    pub wrapper_typ: Option<syn::Path>,

    /// The innermost type of the field.
    pub base_typ: syn::Path,
}

impl ToTokens for PlateableType {
    fn to_tokens(&self, tokens: &mut TokenStream2) {
        let base_typ = self.base_typ.clone();

        if let Some(wrapper) = self.wrapper_typ.clone() {
            tokens.append_all(quote!(#wrapper));
        } else {
            tokens.append_all(quote!(#base_typ));
        }
    }
}

impl HasBaseType for PlateableType {
    fn base_typ(&self) -> syn::Path {
        self.base_typ.clone()
    }
}
