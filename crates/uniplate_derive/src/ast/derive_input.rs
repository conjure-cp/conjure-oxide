#![allow(dead_code)]
#![allow(unused_variables)]

use std::borrow::Borrow;

use syn::bracketed;

use crate::prelude::*;

#[derive(Clone, Debug)]
pub struct DeriveInput {
    pub instance_metadata: Vec<InstanceMeta>,
    pub data: ast::Data,
}

impl Parse for DeriveInput {
    fn parse(input: ParseStream) -> syn::Result<Self> {
        // ATTRIBUTES*
        // DATA_DECLARATION

        let instance_metadata: Vec<InstanceMeta> = input.call(InstanceMeta::parse_many)?;
        let data: ast::Data = input.parse()?;

        Ok(DeriveInput {
            instance_metadata,
            data,
        })
    }
}

/// Parsed metadata associated with a Biplate / Uniplate instance.
///
/// These settings are determined through the #[uniplate(...)] and #[biplate(...)] helper
/// attributes.
#[derive(Clone, Debug)]
pub enum InstanceMeta {
    Uniplate(UniplateInstanceMeta),
    Biplate(BiplateInstanceMeta),
}

pub trait InstanceMetaKind {
    fn from_attribute(attr: syn::Attribute) -> syn::Result<InstanceMeta>;
}

impl InstanceMeta {
    /// Parses 0 or more InstanceMeta attributes.
    pub fn parse_many(input: ParseStream<'_>) -> syn::Result<Vec<InstanceMeta>> {
        // syn parses attributes into vectors, so its easier if we do this aswell!
        let attrs: Vec<syn::Attribute> = input.call(syn::Attribute::parse_outer)?;

        let mut has_uniplate: bool = false;

        let mut instance_metadata: Vec<InstanceMeta> = Vec::new();
        for attr in attrs {
            let Some(attr_name) = attr.path().get_ident() else {
                continue;
            };

            let meta = match attr_name.to_string().borrow() {
                "uniplate" => {
                    if !has_uniplate {
                        has_uniplate = true;
                    } else {
                        return Err(
                            input.error("only one uniplate declaration is expected per type")
                        );
                    };

                    Some(UniplateInstanceMeta::from_attribute(attr)?)
                }
                "biplate" => Some(BiplateInstanceMeta::from_attribute(attr)?),
                _ => None,
            };

            if let Some(meta) = meta {
                instance_metadata.push(meta);
            };
        }
        if !has_uniplate {
            // Default implementation of uniplate without walking into anything
            instance_metadata.push(InstanceMeta::Uniplate(Default::default()));
        }

        Ok(instance_metadata)
    }

    /// Checks if a type can be walked into or not
    pub fn walk_into_type(&self, typ: &ast::Type) -> bool {
        let walk_into = match &self {
            InstanceMeta::Uniplate(u) => &u.walk_into,
            InstanceMeta::Biplate(b) => &b.walk_into,
        };

        for typ2 in walk_into {
            if typ.base_typ() == typ2.base_typ() {
                return true;
            }
        }
        false
    }
}

#[derive(Clone, Debug, Default)]
pub struct UniplateInstanceMeta {
    walk_into: Vec<ast::Type>,
}

impl InstanceMetaKind for UniplateInstanceMeta {
    fn from_attribute(attr: syn::Attribute) -> syn::Result<InstanceMeta> {
        let mut walk_into: Vec<ast::Type> = Vec::new();
        attr.parse_nested_meta(|meta| {
            // #[uniplate(walk_into=(A,B,C))]
            if meta.path.is_ident("walk_into") {
                meta.input.parse::<Token![=]>()?;
                let content;
                bracketed!(content in meta.input);

                let typs: Punctuated<ast::Type, Token![,]> =
                    content.call(Punctuated::parse_terminated)?;
                walk_into.extend(typs.into_iter());
                return Ok(());
            };

            Err(meta.error("unrecognized property"))
        })?;

        Ok(InstanceMeta::Uniplate(UniplateInstanceMeta { walk_into }))
    }
}

#[derive(Clone, Debug)]
pub struct BiplateInstanceMeta {
    pub to: ast::Type,
    pub walk_into: Vec<ast::Type>,
}

impl InstanceMetaKind for BiplateInstanceMeta {
    fn from_attribute(attr: syn::Attribute) -> syn::Result<InstanceMeta> {
        let mut walk_into: Vec<ast::Type> = Vec::new();
        let mut to: Option<ast::Type> = None;
        attr.parse_nested_meta(|meta| {
            // #[biplate(walk_into=(A,B,C))]
            if meta.path.is_ident("walk_into") {
                meta.input.parse::<Token![=]>()?;
                let content;
                bracketed!(content in meta.input);

                let typs: Punctuated<ast::Type, Token![,]> =
                    content.call(Punctuated::parse_terminated)?;
                walk_into.extend(typs.into_iter());
                return Ok(());
            }

            // #[biplate(to=A)]
            if meta.path.is_ident("to") {
                if to.is_some() {
                    return Err(meta.error("only one to type can be given"));
                }
                meta.input.parse::<Token![=]>()?;
                to = Some(meta.input.parse()?);
                return Ok(());
            }

            Err(meta.error("unrecognized property"))
        })?;

        let Some(to) = to else {
            return Err(syn::Error::new(attr.span(), "no to type given"));
        };

        Ok(InstanceMeta::Biplate(BiplateInstanceMeta { to, walk_into }))
    }
}
