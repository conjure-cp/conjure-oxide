use proc_macro2::{Ident, Span};
use syn::spanned::Spanned;
use syn::{Expr, GenericArgument, PathArguments, PathSegment, Type};

pub enum ParseTypeArgumentError {
    NoTypeArguments,
    EmptyTypeArguments,
    MultipleTypeArguments,
    TypeArgumentNotAType,
    TypeArgumentValueNotPath,
    TypeArgumentEmptyPath,
}

#[derive(Debug)]
pub enum UniplateField {
    Identifier(Ident),
    Box(Span, Box<UniplateField>),
    Vector(Span, Box<UniplateField>),
    Tuple(Span, Vec<UniplateField>),
    Array(Span, Box<UniplateField>, Expr),
    Unknown(Span),
}

impl UniplateField {
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

pub fn parse_field_type(field_type: &Type) -> UniplateField {
    fn parse_type(seg: &PathSegment) -> UniplateField {
        let ident = &seg.ident;
        let span = ident.span();
        let args = &seg.arguments;

        let box_ident = &Ident::new("Box", span);
        let vec_ident = &Ident::new("Vec", span);

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
