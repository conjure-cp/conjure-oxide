use crate::prelude::*;
use itertools::Itertools;

/// All valid field wrapper types - e.g Box, Vec, ...
#[derive(Clone, Debug)]
pub enum WrapperTypes {
    Box,
    Vec,
    Option,
    None,
}

#[derive(Clone, Debug)]
pub enum Type {
    Plateable(PlateableType),
    Unplateable,
}

impl Parse for Type {
    fn parse(input: syn::parse::ParseStream) -> syn::Result<Self> {
        let typ: syn::Type = input.parse()?;
        let syn::Type::Path(typ) = typ else {
            return Ok(Type::Unplateable);
        };

        // all possible names for types that we are interested  in

        //TODO: lazystatic this?
        let box_strings = ["::std::boxed::Box", "std::boxed::Box", "Box"];
        let vec_strings = [
            "std::Vec",
            "std::vec::Vec",
            "::std::Vec",
            "::std::vec::Vec",
            "Vec",
        ];
        let option_strings = [
            "std::Option",
            "Option",
            "core::Option",
            "::std::Option",
            "::core::Option",
        ];

        let type_str: String = typ
            .path
            .segments
            .iter()
            .map(|x| x.ident.to_string())
            .intersperse("::".to_owned())
            .collect();
        let last_segment = typ.path.segments.last().expect("");
        let wrapper_ty: WrapperTypes = if box_strings.contains(&type_str.as_str()) {
            WrapperTypes::Box
        } else if vec_strings.contains(&type_str.as_str()) {
            WrapperTypes::Vec
        } else if option_strings.contains(&type_str.as_str()) {
            WrapperTypes::Option
        } else {
            // Cannot have a generic type for now
            let syn::PathArguments::None = last_segment.arguments else {
                return Err(syn::Error::new(
                    last_segment.span(),
                    "Biplate: types with parameters are not supported",
                ));
            };
            return Ok(Type::Plateable(PlateableType {
                span: typ.span(),
                base_typ: typ.path,
                wrapper_typ: WrapperTypes::None,
            }));
        };
        // Check inside the angle brackets for the inner type
        let syn::PathArguments::AngleBracketed(param) = last_segment.arguments.clone() else {
            return Err(syn::Error::new(
                last_segment.span(),
                "Biplate: expected <> here",
            ));
        };

        if (param.args.len() != 1) {
            // should never happen!
            return Err(syn::Error::new(
                param.args.span(),
                "Biplate: only expected one generic argument here.",
            ));
        }
        let syn::GenericArgument::Type(syn::Type::Path(base_typ)) = param.args.first().expect("")
        else {
            // should never happen!
            return Err(syn::Error::new(
                param.args.span(),
                "Biplate: expected a type here.",
            ));
        };

        Ok(Type::Plateable(PlateableType {
            span: typ.span(),
            base_typ: base_typ.path.clone(),
            wrapper_typ: wrapper_ty,
        }))
    }
}

#[derive(Clone, Debug)]
pub struct PlateableType {
    /// The underlying type of the field.
    pub base_typ: syn::Path,

    /// The wrapper type of the field.
    pub wrapper_typ: WrapperTypes,

    pub span: Span,
}
