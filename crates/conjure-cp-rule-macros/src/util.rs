use syn::visit_mut::VisitMut;
use syn::{Ident, Type};

/// Check whether a type AST contains a reference to a specific ident (the generic param).
pub fn type_contains_ident(ty: &Type, ident: &Ident) -> bool {
    match ty {
        Type::Path(tp) => {
            // Check if it's just the ident itself (e.g. `T`)
            if tp.qself.is_none()
                && tp.path.segments.len() == 1
                && tp.path.segments[0].ident == *ident
                && tp.path.segments[0].arguments.is_empty()
            {
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

/// Replace all occurrences of `ident` with `replacement` in a type, returning the modified Type.
pub fn replace_ident_in_type(mut ty: Type, ident: &Ident, replacement: &str) -> Type {
    let mut replacer = IdentReplacer {
        target: ident.to_string(),
        replacement: Ident::new(replacement, ident.span()),
    };
    replacer.visit_type_mut(&mut ty);
    ty
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
