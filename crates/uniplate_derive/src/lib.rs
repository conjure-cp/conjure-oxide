mod ast;
mod prelude;
mod state;

use std::collections::VecDeque;

use prelude::*;
use quote::format_ident;
use syn::parse_macro_input;

#[proc_macro_derive(Uniplate, attributes(uniplate, biplate))]
pub fn uniplate_derive(input: TokenStream) -> TokenStream {
    let input = parse_macro_input!(input as ast::DeriveInput);
    //eprintln!("{:#?}",input.clone());
    let mut state: ParserState = ParserState::new(input.clone());

    let mut out_tokens: Vec<TokenStream2> = Vec::new();
    out_tokens.push(quote! {
        use std::borrow::Borrow as _;
    });

    while state.next_instance().is_some() {
        out_tokens.push(match &state.current_instance {
            Some(ast::InstanceMeta::Uniplate(_)) => derive_a_uniplate(&mut state),
            Some(ast::InstanceMeta::Biplate(_)) => derive_a_biplate(&mut state),
            _ => unreachable!(),
        });
    }

    out_tokens.into_iter().collect::<TokenStream2>().into()
}

fn derive_a_uniplate(state: &mut ParserState) -> TokenStream2 {
    let from = state.from.to_token_stream();
    let tokens: TokenStream2 = match state.data.clone() {
        ast::Data::DataEnum(x) => _derive_a_enum_uniplate(state, x),
    };

    quote! {
        impl ::uniplate::biplate::Uniplate for #from {
            fn uniplate(&self) -> (::uniplate::Tree<#from>, Box<dyn Fn(::uniplate::Tree<#from>) -> #from>) {
                #tokens
            }
        }
    }
}

fn _derive_a_enum_uniplate(state: &mut ParserState, data: ast::DataEnum) -> TokenStream2 {
    let mut variant_tokens = VecDeque::<TokenStream2>::new();
    for variant in data.variants {
        if variant.fields.is_empty() {
            let ident = variant.ident;
            let enum_ident = state.data.ident();
            variant_tokens.push_back(quote! {
                #enum_ident::#ident => {
                    (::uniplate::Tree::Zero,Box::new(|_| #enum_ident::#ident))
                },
            });
        } else {
            let field_idents: Vec<syn::Ident> = variant
                .fields
                .iter()
                .enumerate()
                .map(|(i, _)| format_ident!("_f{}", i))
                .collect();
            let field_defs: Vec<_> = std::iter::zip(variant.fields.clone(), field_idents.clone())
                .map(|(field, ident)| _derive_for_field(state, field, ident))
                .collect();
            let children_def = _derive_children(state, &variant.fields);
            let ctx_def = _derive_ctx(state, &variant.fields, &variant.ident);
            let ident = variant.ident;
            let enum_ident = state.data.ident();
            variant_tokens.push_back(quote! {
                #enum_ident::#ident(#(#field_idents),*) => {
                    #(#field_defs)*

                    #children_def

                    #ctx_def

                    (children,ctx)
                },
            });
        }
    }

    let variant_tokens = variant_tokens.iter();
    quote! {
        match self {
            #(#variant_tokens)*
        }
    }
}

fn _derive_for_field(
    state: &mut ParserState,
    field: ast::Field,
    ident: syn::Ident,
) -> TokenStream2 {
    let children_ident = format_ident!("{}_children", ident);
    let ctx_ident = format_ident!("{}_ctx", ident);

    let to_t = state.to.clone().expect("").to_token_stream();

    if !state.walk_into_type(&field.typ) {
        let copy_ident = format_ident!("{}_copy", ident);
        quote! {
            let #copy_ident = #ident.clone();
        }
    } else {
        match &field.typ {
            // dereference the field
            ast::Type::BoxedPlateable(x) => {
                let from_t = x.inner_typ.to_token_stream();
                quote! {
                    let (#children_ident,#ctx_ident) = <#from_t as ::uniplate::biplate::Biplate<#to_t>>::biplate(#ident.borrow());
                }
            }
            ast::Type::Plateable(x) => {
                let from_t = x.to_token_stream();
                quote! {
                    let (#children_ident,#ctx_ident) = <#from_t as ::uniplate::biplate::Biplate<#to_t>>::biplate(#ident);
                }
            }
            ast::Type::Unplateable => {
                let copy_ident = format_ident!("{}_copy", ident);
                quote! {
                    let #copy_ident = #ident.clone();
                }
            }
        }
    }
}

fn _derive_children(state: &mut ParserState, fields: &[ast::Field]) -> TokenStream2 {
    let mut subtrees: VecDeque<TokenStream2> = VecDeque::new();
    for (i, field) in fields.iter().enumerate() {
        if !state.walk_into_type(&field.typ) {
            subtrees.push_back(quote!(::uniplate::Tree::Zero));
            continue;
        }
        subtrees.push_back(match field.typ {
            ast::Type::BoxedPlateable(_) => {
                let children_ident = format_ident!("_f{}_children", i);
                quote!(#children_ident)
            }
            ast::Type::Plateable(_) => {
                let children_ident = format_ident!("_f{}_children", i);
                quote!(#children_ident)
            }
            ast::Type::Unplateable => quote!(::uniplate::Tree::Zero),
        });
    }

    match subtrees.len() {
        0 => quote! {let children = ::uniplate::Tree::Zero;},
        _ => {
            let subtrees = subtrees.iter();
            quote! {let children = ::uniplate::Tree::Many(::im::vector![#(#subtrees),*]);}
        }
    }
}

fn _derive_ctx(
    state: &mut ParserState,
    fields: &[ast::Field],
    var_ident: &syn::Ident,
) -> TokenStream2 {
    let field_ctxs: Vec<_> = fields
        .iter()
        .enumerate()
        .map(|(i, f)| {
            if !state.walk_into_type(&f.typ) {
                let ident = format_ident!("_f{}_copy", i);
                quote! {#ident.clone()}
            } else {
                match &f.typ {
                    ast::Type::Unplateable => {
                        let ident = format_ident!("_f{}_copy", i);
                        quote! {#ident.clone()}
                    }

                    ast::Type::Plateable(_) => {
                        let ctx_ident = format_ident!("_f{}_ctx", i);
                        quote! {#ctx_ident(x[#i].clone())}
                    }

                    ast::Type::BoxedPlateable(x) => {
                        let boxed_typ = x.box_typ.to_token_stream();
                        let ctx_ident = format_ident!("_f{}_ctx", i);
                        quote! {#boxed_typ::new(#ctx_ident(x[#i].clone()))}
                    }
                }
            }
        })
        .collect();

    let data_ident = state.data.ident();
    let typ = state.to.clone();
    match fields.len() {
        0 => {
            quote! {
                let ctx = Box::new(move |x: ::uniplate::Tree<#typ>| {
                    let ::uniplate::Tree::Zero = x else { panic!()};
                    #data_ident::#var_ident
                });
            }
        }
        _ => {
            quote! {
                    let ctx = Box::new(move |x: ::uniplate::Tree<#typ>| {
                        let ::uniplate::Tree::Many(x) = x else { panic!()};
                        #data_ident::#var_ident(#(#field_ctxs),*)
            });}
        }
    }
}

fn derive_a_biplate(state: &mut ParserState) -> TokenStream2 {
    let from = state.from.base_typ.to_token_stream();
    let to = state.to.to_token_stream();

    if from.to_string() == to.to_string() {
        return _derive_identity_biplate(from);
    }

    let tokens: TokenStream2 = match state.data.clone() {
        ast::Data::DataEnum(x) => _derive_a_enum_uniplate(state, x),
    };

    quote! {
        impl ::uniplate::biplate::Biplate<#to> for #from {
            fn biplate(&self) -> (::uniplate::Tree<#to>, Box<dyn Fn(::uniplate::Tree<#to>) -> #from>) {
                #tokens
            }
        }
    }
}

fn _derive_identity_biplate(typ: TokenStream2) -> TokenStream2 {
    quote! {
        impl ::uniplate::biplate::Biplate<#typ> for #typ{
            fn biplate(&self) -> (::uniplate::Tree<#typ>, Box<dyn Fn(::uniplate::Tree<#typ>) -> #typ>) {
                let val = self.clone();
                (::uniplate::Tree::One(val.clone()),Box::new(move |x| {
                    let ::uniplate::Tree::One(x) = x else {todo!()};
                    x
                }))
            }
        }
    }
}
