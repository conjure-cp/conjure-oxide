use proc_macro2::{Ident, Span};
use syn::spanned::Spanned;
use syn::{Expr, GenericArgument, PathArguments, PathSegment, Type};

/// Represents an error produced during parsing a type argument (e.g. `::<T>`)
pub enum ParseTypeArgumentError {
    NoTypeArguments,
    EmptyTypeArguments,
    MultipleTypeArguments,
    TypeArgumentNotAType,
    TypeArgumentValueNotPath,
    TypeArgumentEmptyPath,
}

/// Represents a field in a tree-like structure. Used for deriving the uniplate implementation.
#[derive(Debug)]
pub enum UniplateField {
    /// Any other valid identifier
    Identifier(Ident),
    /// A field consisting of a Box<T>
    Box(Span, Box<UniplateField>),
    /// A field consisting of a Vec<T>
    Vector(Span, Box<UniplateField>),
    /// A tuple of multiple fields (e.g. `(Box<T>, i32)`)
    Tuple(Span, Vec<UniplateField>),
    /// An array field. ToDo: currently not supported.
    Array(Span, Box<UniplateField>, Expr),
    /// A field that could not be parsed
    Unknown(Span),
}

impl UniplateField {
    /// Get the span corresponding to this field
    pub fn span(&self) -> Span {
        match self {
            UniplateField::Identifier(idnt) => idnt.span(),
            UniplateField::Box(spn, _) => *spn,
            UniplateField::Vector(spn, _) => *spn,
            UniplateField::Tuple(spn, _) => *spn,
            UniplateField::Array(spn, _, _) => *spn,
            UniplateField::Unknown(spn) => *spn,
        }
    }
}

/// Parse a type argument from a path segment (e.g. `T` from `Box<T>`)
fn parse_type_argument(seg_args: &PathArguments) -> Result<&PathSegment, ParseTypeArgumentError> {
    match seg_args {
        PathArguments::AngleBracketed(type_args) => {
            if type_args.args.len() > 1 {
                // ToDo: discuss - can and should we support multiple type arguments?
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

/// Parse a field type into a `UniplateField`
pub fn parse_field_type(field_type: &Type) -> UniplateField {
    /// Helper function to parse a path segment into a `UniplateField`
    fn parse_type(seg: &PathSegment) -> UniplateField {
        let ident = &seg.ident;
        let span = ident.span();
        let args = &seg.arguments;

        let box_ident = &Ident::new("Box", span);
        let vec_ident = &Ident::new("Vec", span); // ToDo: support other collection types

        if ident.eq(box_ident) {
            match parse_type_argument(args) {
                Ok(inner_seg) => UniplateField::Box(seg.span(), Box::new(parse_type(inner_seg))),
                Err(_) => UniplateField::Unknown(ident.span()),
            }
        } else if ident.eq(vec_ident) {
            match parse_type_argument(args) {
                Ok(inner_seg) => UniplateField::Vector(seg.span(), Box::new(parse_type(inner_seg))),
                Err(_) => UniplateField::Unknown(ident.span()),
            }
        } else {
            UniplateField::Identifier(ident.clone())
        }
    }

    match field_type {
        Type::Path(path) => match path.path.segments.last() {
            None => UniplateField::Unknown(path.span()),
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
        _ => UniplateField::Unknown(field_type.span()), // ToDo discuss - Can we support any of: BareFn, Group, ImplTrait, Infer, Macro, Never, Paren, Ptr, Reference, TraitObject, Verbatim
    }
}

/// Check if a field type is equal to a given identifier. Used to check if a field is an instance of the root type.
pub fn check_field_type(ft: &UniplateField, root_ident: &Ident) -> bool {
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
