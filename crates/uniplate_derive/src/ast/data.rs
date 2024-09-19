use syn::token;

use crate::prelude::*;

#[derive(Clone, Debug)]
pub enum Data {
    DataEnum(DataEnum),
}

impl Data {
    #[allow(dead_code)]
    pub fn span(&self) -> Span {
        match self {
            Data::DataEnum(x) => x.span,
        }
    }

    pub fn ident(&self) -> syn::Ident {
        match self {
            Data::DataEnum(x) => x.ident.clone(),
        }
    }
}

impl From<Data> for ast::PlateableType {
    fn from(val: Data) -> Self {
        match val {
            Data::DataEnum(x) => {
                let mut typ_segments: Punctuated<syn::PathSegment, syn::token::PathSep> =
                    Punctuated::new();
                typ_segments.push(syn::PathSegment {
                    ident: x.ident,
                    arguments: syn::PathArguments::None,
                });

                let base_typ: syn::Path = syn::Path {
                    leading_colon: None,
                    segments: typ_segments,
                };

                ast::PlateableType {
                    base_typ,
                    wrapper_typ: None,
                }
            }
        }
    }
}

impl Parse for Data {
    fn parse(input: ParseStream) -> syn::Result<Self> {
        input.parse::<syn::Visibility>()?;
        let lookahead = input.lookahead1();
        if lookahead.peek(Token![enum]) {
            input.parse().map(Data::DataEnum)
        } else {
            Err(lookahead.error())
        }
    }
}

#[derive(Clone, Debug)]
pub struct DataEnum {
    pub ident: syn::Ident,
    pub span: Span,
    pub variants: Vec<Variant>,
}
impl Parse for DataEnum {
    // Layout of an enum as per:
    // https://doc.rust-lang.org/stable/reference/items/enumerations.html
    // https://docs.rs/syn/latest/syn/struct.ItemEnum.html

    fn parse(input: ParseStream) -> syn::Result<Self> {
        //input.parse::<syn::Visibility>()?;
        input.parse::<Token![enum]>()?;
        let ident = input.parse::<syn::Ident>()?;

        input.parse::<syn::Generics>()?;

        let content;
        braced! {content in input};

        let variants: Punctuated<Variant, Token![,]> =
            content.parse_terminated(Variant::parse, Token![,])?;

        Ok(DataEnum {
            span: ident.span(),
            ident,
            variants: variants.into_iter().collect(),
        })
    }
}

#[derive(Clone, Debug)]
pub struct Variant {
    pub ident: syn::Ident,
    #[allow(dead_code)]
    pub span: Span,
    pub fields: Vec<Field>,
}
impl Parse for Variant {
    fn parse(input: ParseStream) -> syn::Result<Self> {
        // Layout of a variant as per:
        // https://docs.rs/syn/latest/syn/struct.Variant.html
        // https://doc.rust-lang.org/stable/reference/items/enumerations.html

        input.call(syn::Attribute::parse_outer)?;
        let ident: syn::Ident = input.parse()?;

        if !input.peek(token::Paren) {
            return Ok(Variant {
                span: ident.span(),
                ident,
                fields: Default::default(),
            });
        }

        let content;
        parenthesized! {content in input};

        let fields: Punctuated<Field, Token![,]> = content.call(Punctuated::parse_terminated)?;
        Ok(Variant {
            span: ident.span(),
            ident,
            fields: fields.into_iter().collect(),
        })
    }
}

#[derive(Clone, Debug)]
pub struct Field {
    #[allow(dead_code)]
    pub span: Span,
    pub typ: ast::Type,
}

impl Parse for Field {
    fn parse(input: ParseStream) -> syn::Result<Self> {
        // Layout of a field as per:
        // https://docs.rs/syn/latest/syn/struct.Field.html
        // https://doc.rust-lang.org/stable/reference/items/structs.html (tuple field)
        input.call(syn::Attribute::parse_outer)?;
        input.parse::<syn::Visibility>()?;
        let span = input.span();
        let typ: ast::Type = input.parse()?;
        Ok(Field { span, typ })
    }
}
