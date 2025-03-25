use quote::{quote, ToTokens};
use uniplate::{Biplate, Uniplate};

use super::{AbstractLiteral, Atom, Domain, Expression, Literal, Name, Range, SetAttr};

fn vec_to_tokens<T: ToTokens>(vec: &Vec<T>) -> proc_macro2::TokenStream {
    quote! { vec![#(#vec),*] }
}

impl ToTokens for Name {
    fn to_tokens(&self, tokens: &mut proc_macro2::TokenStream) {
        match self {
            Name::UserName(ident) => tokens.extend(quote! {
                conjure_core::ast::name::Name::Identifier(#ident)
            }),
            Name::MachineName(ident) => tokens.extend(quote! {
                conjure_core::ast::name::Name::MachineName(#ident)
            }),
            Name::RepresentedName(src, rule, extra) => tokens.extend(quote! {
                conjure_core::ast::name::Name::RepresentedName(#src, #rule, #extra)
            }),
            Name::WithRepresentation(src, reps) => {
                let rep_toks = vec_to_tokens(reps);
                tokens.extend(quote! {
                    conjure_core::ast::name::Name::WithRepresentation(#src, #rep_toks)
                });
            }
        }
    }
}

impl<T: ToTokens + Ord> ToTokens for Range<T> {
    fn to_tokens(&self, tokens: &mut proc_macro2::TokenStream) {
        match self {
            Range::Single(a) => tokens.extend(quote! {
                conjure_core::ast::domains::Range::Single(#a)
            }),
            Range::Bounded(a, b) => tokens.extend(quote! {
                conjure_core::ast::domains::Range::Bounded(#a, #b)
            }),
            Range::UnboundedL(a) => tokens.extend(quote! {
                conjure_core::ast::domains::Range::UnboundedL(#a)
            }),
            Range::UnboundedR(b) => tokens.extend(quote! {
                conjure_core::ast::domains::Range::UnboundedR(#b)
            }),
        }
    }
}

impl ToTokens for SetAttr {
    fn to_tokens(&self, tokens: &mut proc_macro2::TokenStream) {
        match self {
            SetAttr::None => tokens.extend(quote! {
                conjure_core::ast::domains::SetAttr::None
            }),
            SetAttr::Size(a) => tokens.extend(quote! {
                conjure_core::ast::domains::SetAttr::Size(#a)
            }),
            SetAttr::MinSize(a) => tokens.extend(quote! {
                conjure_core::ast::domains::SetAttr::MinSize(#a)
            }),
            SetAttr::MaxSize(a) => tokens.extend(quote! {
                conjure_core::ast::domains::SetAttr::MaxSize(#a)
            }),
            SetAttr::MinMaxSize(a, b) => tokens.extend(quote! {
                conjure_core::ast::domains::SetAttr::MinMaxSize(#a, #b)
            }),
        }
    }
}

impl ToTokens for Domain {
    fn to_tokens(&self, tokens: &mut proc_macro2::TokenStream) {
        match self {
            Domain::BoolDomain => tokens.extend(quote! {
                conjure_core::ast::domains::Domain::BoolDomain
            }),
            Domain::IntDomain(ranges) => {
                let range_toks = vec_to_tokens(ranges);
                tokens.extend(quote! {
                    conjure_core::ast::domains::Domain::IntDomain(#range_toks)
                });
            }
            Domain::DomainReference(name) => tokens.extend(quote! {
                conjure_core::ast::domains::Domain::DomainReference(#name)
            }),
            Domain::DomainSet(attrs, domain) => tokens.extend(quote! {
                conjure_core::ast::domains::Domain::DomainSet(#attrs, #domain)
            }),
            Domain::DomainMatrix(val, idx) => {
                let idx_toks = vec_to_tokens(idx);
                tokens.extend(quote! {
                    conjure_core::ast::domains::Domain::DomainMatrix(#val, #idx_toks)
                });
            }
        }
    }
}

impl<T> ToTokens for AbstractLiteral<T>
where
    T: Uniplate + Biplate<AbstractLiteral<T>> + Biplate<T> + ToTokens,
{
    fn to_tokens(&self, tokens: &mut proc_macro2::TokenStream) {
        match self {
            AbstractLiteral::Set(items) => {
                let item_toks = vec_to_tokens(items);
                tokens.extend(quote! {
                    conjure_core::ast::AbstractLiteral::Set(#item_toks)
                });
            }
            AbstractLiteral::Matrix(items, domain) => {
                let item_toks = vec_to_tokens(items);
                tokens.extend(quote! {
                    conjure_core::ast::AbstractLiteral::Matrix(#item_toks, #domain)
                });
            }
        }
    }
}

impl ToTokens for Literal {
    fn to_tokens(&self, tokens: &mut proc_macro2::TokenStream) {
        match self {
            Literal::Int(x) => tokens.extend(quote! {
                conjure_core::ast::Literal::Int(#x)
            }),
            Literal::Bool(x) => tokens.extend(quote! {
                conjure_core::ast::Literal::Bool(#x)
            }),
            Literal::AbstractLiteral(x) => tokens.extend(quote! {
                conjure_core::ast::Literal::AbstractLiteral(#x)
            }),
        }
    }
}

impl ToTokens for Atom {
    fn to_tokens(&self, tokens: &mut proc_macro2::TokenStream) {
        match self {
            Atom::Reference(name) => tokens.extend(quote! {
                conjure_core::ast::Atom::Reference(#name)
            }),
            Atom::Literal(lit) => tokens.extend(quote! {
                conjure_core::ast::Atom::Literal(#lit)
            }),
        }
    }
}

// impl ToTokens for Expression {
//     fn to_tokens(&self, tokens: &mut proc_macro2::TokenStream) {
//         match self {
//             Expression::Eq(m, l, r) => todo!("Implement Expression::Eq"),
//             Expression::And(m, v) => todo!("Implement Expression::And"),
//             Expression::Or(m, v) => todo!("Implement Expression::Or"),
//             Expression::Not(m, v) => todo!("Implement Expression::Not"),
//             Expression::UnsafeSlice(m, e, idx) => todo!("Implement Expression::UnsafeSlice"),
//             Expression::Gt(m, l, r) => todo!("Implement Expression::Gt"),
//             Expression::Lt(m, l, r) => todo!("Implement Expression::Lt"),
//             Expression::Geq(m, l, r) => todo!("Implement Expression::Gte"),
//             Expression::Leq(m, l, r) => todo!("Implement Expression::Lte"),
//             Expression::AbstractLiteral(m, a) => todo!("Implement Expression::AbstractLiteral"),
//             Expression::Abs(m, x) => todo!("Implement Expression::Abs"),
//             Expression::Root(m, x) => todo!("Implement Expression::Root"),
//             Expression::Bubble(m, l, r) => todo!("Implement Expression::Bubble"),
//             Expression::DominanceRelation(m, r) => todo!("Implement Expression::DominanceRelation"),
//             Expression::FromSolution(m, n) => todo!("Implement Expression::FromSolution"),
//             Expression::Metavar(m, n) => todo!("Implement Expression::Metavar"),
//             Expression::Atomic(m, a) => todo!("Implement Expression::Atomic"),
//             Expression::UnsafeIndex(m, e, idx) => todo!("Implement Expression::UnsafeIndex"),
//             Expression::SafeIndex(m, e, idx) => todo!("Implement Expression::SafeIndex"),
//             Expression::SafeSlice(m, e, idx) => todo!("Implement Expression::SafeSlice"),
//             Expression::InDomain(n, e, d) => todo!("Implement Expression::InDomain"),
//             Expression::Scope(m, sm) => todo!("Implement Expression::Scope"),
//             Expression::Sum(m, v) => todo!("Implement Expression::Sum"),
//             Expression::Product(m, v) => todo!("Implement Expression::Product"),
//             Expression::Min(m, v) => todo!("Implement Expression::Min"),
//             Expression::Max(m, v) => todo!("Implement Expression::Max"),
//             Expression::Imply(m, l, r) => todo!("Implement Expression::Imply"),
//             Expression::Neq(m, l, r) => todo!("Implement Expression::Neq"),
//             Expression::SafeDiv(m, l, r) => todo!("Implement Expression::SafeDiv"),
//             Expression::SafeMod(m, l, r) => todo!("Implement Expression::SafeMod"),
//             Expression::UnsafeDiv(m, l, r) => todo!("Implement Expression::UnsafeDiv"),
//         }
//     }
// }
