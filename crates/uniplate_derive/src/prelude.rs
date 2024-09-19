//! Common imports for all files in this project

pub(crate) use crate::ast;
pub(crate) use crate::state::*;
pub(crate) use proc_macro::TokenStream;
pub(crate) use proc_macro2::Span;
pub(crate) use proc_macro2::TokenStream as TokenStream2;
pub(crate) use quote::quote;
pub(crate) use quote::ToTokens;

pub(crate) use syn::braced;
pub(crate) use syn::parenthesized;
pub(crate) use syn::parse::Parse;
pub(crate) use syn::parse::ParseStream;
pub(crate) use syn::punctuated::Punctuated;
pub(crate) use syn::spanned::Spanned;
pub(crate) use syn::Token;
