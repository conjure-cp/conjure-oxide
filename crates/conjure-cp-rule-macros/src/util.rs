use syn::visit_mut::VisitMut;
use syn::{
    GenericArgument, Ident, PathArguments, ReturnType, Type, TypePath, parse_quote,
    punctuated::Punctuated,
};

pub fn type_is_ident(ty: &Type, ident: &Ident) -> bool {
    match ty {
        Type::Path(tp) => type_path_is_ident(tp, ident),
        _ => false,
    }
}

/// Check whether a type path *is* exactly a given ident
pub fn type_path_is_ident(tp: &TypePath, ident: &Ident) -> bool {
    tp.qself.is_none()
        && tp.path.segments.len() == 1
        && tp.path.segments[0].ident == *ident
        && tp.path.segments[0].arguments.is_empty()
}

/// Check whether a type AST contains a reference to a specific ident
pub fn type_contains_ident(ty: &Type, ident: &Ident) -> bool {
    match ty {
        Type::Path(tp) => {
            if type_path_is_ident(tp, ident) {
                return true;
            }

            // Check type arguments recursively (e.g. `Vec<T>`, `Option<Vec<T>>`)
            tp.path.segments.iter().any(|seg| {
                if seg.ident == *ident {
                    return true;
                }
                match &seg.arguments {
                    syn::PathArguments::AngleBracketed(args) => {
                        args.args.iter().any(|arg| match arg {
                            syn::GenericArgument::Type(inner_ty) => {
                                type_contains_ident(inner_ty, ident)
                            }
                            _ => false,
                        })
                    }
                    syn::PathArguments::Parenthesized(args) => args
                        .inputs
                        .iter()
                        .any(|inner_ty| type_contains_ident(inner_ty, ident)),
                    syn::PathArguments::None => false,
                }
            })
        }
        Type::Tuple(tt) => tt.elems.iter().any(|t| type_contains_ident(t, ident)),
        Type::Array(ta) => type_contains_ident(&ta.elem, ident),
        Type::Slice(ts) => type_contains_ident(&ts.elem, ident),
        Type::Reference(tr) => type_contains_ident(&tr.elem, ident),
        Type::Paren(tp) => type_contains_ident(&tp.elem, ident),
        _ => false,
    }
}

/// Build a `serde_as` adapter type for `ty`, replacing occurrences of `ident` with
/// `replacement` and using `_` in adapter argument positions for non-target types.
pub fn build_serde_as_type(ty: &Type, ident: &Ident, replacement: &str) -> Type {
    let replacement_ident = Ident::new(replacement, ident.span());
    build_serde_as_type_inner(ty, ident, &replacement_ident, false)
}

fn build_serde_as_type_inner(
    ty: &Type,
    ident: &Ident,
    replacement: &Ident,
    in_adapter_position: bool,
) -> Type {
    if !type_contains_ident(ty, ident) {
        return if in_adapter_position {
            parse_quote!(_)
        } else {
            ty.clone()
        };
    }

    match ty {
        Type::Path(tp) if type_path_is_ident(tp, ident) => {
            parse_quote!(#replacement)
        }
        Type::Path(tp) => {
            let mut out = tp.clone();
            for seg in &mut out.path.segments {
                match &seg.arguments {
                    PathArguments::AngleBracketed(args) => {
                        let rewritten: Punctuated<GenericArgument, syn::token::Comma> = args
                            .args
                            .iter()
                            .map(|arg| rewrite_generic_arg(arg, ident, replacement))
                            .collect();
                        let mut new_args = args.clone();
                        new_args.args = rewritten;
                        seg.arguments = PathArguments::AngleBracketed(new_args);
                    }
                    PathArguments::Parenthesized(args) => {
                        let mut new_args = args.clone();
                        new_args.inputs = args
                            .inputs
                            .iter()
                            .map(|input| build_serde_as_type_inner(input, ident, replacement, true))
                            .collect();
                        new_args.output = match &args.output {
                            ReturnType::Default => ReturnType::Default,
                            ReturnType::Type(arrow, ty) => ReturnType::Type(
                                *arrow,
                                Box::new(build_serde_as_type_inner(ty, ident, replacement, true)),
                            ),
                        };
                        seg.arguments = PathArguments::Parenthesized(new_args);
                    }
                    PathArguments::None => {}
                }
            }
            Type::Path(out)
        }
        Type::Tuple(tuple) => {
            let mut out = tuple.clone();
            out.elems = tuple
                .elems
                .iter()
                .map(|elem| build_serde_as_type_inner(elem, ident, replacement, true))
                .collect();
            Type::Tuple(out)
        }
        Type::Array(array) => {
            let mut out = array.clone();
            out.elem = Box::new(build_serde_as_type_inner(
                &array.elem,
                ident,
                replacement,
                true,
            ));
            Type::Array(out)
        }
        Type::Slice(slice) => {
            let mut out = slice.clone();
            out.elem = Box::new(build_serde_as_type_inner(
                &slice.elem,
                ident,
                replacement,
                true,
            ));
            Type::Slice(out)
        }
        Type::Reference(reference) => {
            let mut out = reference.clone();
            out.elem = Box::new(build_serde_as_type_inner(
                &reference.elem,
                ident,
                replacement,
                in_adapter_position,
            ));
            Type::Reference(out)
        }
        Type::Paren(paren) => {
            let mut out = paren.clone();
            out.elem = Box::new(build_serde_as_type_inner(
                &paren.elem,
                ident,
                replacement,
                in_adapter_position,
            ));
            Type::Paren(out)
        }
        _ => {
            if in_adapter_position {
                parse_quote!(_)
            } else {
                ty.clone()
            }
        }
    }
}

fn rewrite_generic_arg(
    arg: &GenericArgument,
    ident: &Ident,
    replacement: &Ident,
) -> GenericArgument {
    match arg {
        GenericArgument::Type(ty) => {
            GenericArgument::Type(build_serde_as_type_inner(ty, ident, replacement, true))
        }
        GenericArgument::AssocType(assoc_ty) => {
            let mut out = assoc_ty.clone();
            out.ty = build_serde_as_type_inner(&assoc_ty.ty, ident, replacement, true);
            GenericArgument::AssocType(out)
        }
        _ => arg.clone(),
    }
}

/// Rename an `ItemFn`'s identifier, returning the modified function.
pub fn rename_fn(mut func: syn::ItemFn, new_name: &Ident) -> syn::ItemFn {
    func.sig.ident = new_name.clone();
    func
}

/// Rename all occurrences of `from` to `to` within an `ItemFn` (signature + body).
pub fn rename_ident_in_fn(mut func: syn::ItemFn, from: &Ident, to: &Ident) -> syn::ItemFn {
    let mut replacer = IdentReplacer {
        target: from.to_string(),
        replacement: to.clone(),
    };
    VisitMut::visit_item_fn_mut(&mut replacer, &mut func);
    func
}

/// Rename all occurrences of `from` to `to` within an `ItemImpl`
pub fn rename_ident_in_impl(mut item: syn::ItemImpl, from: &Ident, to: &Ident) -> syn::ItemImpl {
    let mut replacer = IdentReplacer {
        target: from.to_string(),
        replacement: to.clone(),
    };
    VisitMut::visit_item_impl_mut(&mut replacer, &mut item);
    item
}

/// A `VisitMut` that replaces all idents matching `target` with `replacement`.
struct IdentReplacer {
    target: String,
    replacement: Ident,
}

impl VisitMut for IdentReplacer {
    fn visit_ident_mut(&mut self, i: &mut Ident) {
        if *i == self.target {
            *i = self.replacement.clone();
        }
    }
}

#[cfg(test)]
mod tests {
    use super::build_serde_as_type;
    use quote::ToTokens;
    use syn::{Ident, Type, parse_quote};

    fn assert_adapter_type(input: Type, expected: Type) {
        let generic = Ident::new("T", proc_macro2::Span::call_site());
        let actual = build_serde_as_type(&input, &generic, "ReprStateSerde");

        assert_eq!(
            actual.to_token_stream().to_string(),
            expected.to_token_stream().to_string()
        );
    }

    #[test]
    fn map_value_uses_repr_state_and_passthrough_key() {
        assert_adapter_type(
            parse_quote!(HashMap<Name, T>),
            parse_quote!(HashMap<_, ReprStateSerde>),
        );
    }

    #[test]
    fn nested_collections_are_rewritten_recursively() {
        assert_adapter_type(
            parse_quote!(Vec<HashMap<Name, Vec<T>>>),
            parse_quote!(Vec<HashMap<_, Vec<ReprStateSerde>>>),
        );
    }

    #[test]
    fn tuple_positions_use_passthrough_for_non_generic_elements() {
        assert_adapter_type(
            parse_quote!((Name, Option<T>)),
            parse_quote!((_, Option<ReprStateSerde>)),
        );
    }
}
