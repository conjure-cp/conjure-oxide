use quote::quote;
use syn::{parse2, ItemFn};

// Documentation in the lib.rs file of the public facing conjure_oxide crate.

#[proc_macro_attribute]
pub fn rule(
    args: proc_macro::TokenStream,
    item: proc_macro::TokenStream,
) -> proc_macro::TokenStream {
    let args2 = proc_macro2::TokenStream::from(args);
    let item2 = proc_macro2::TokenStream::from(item);

    let item_parsed: ItemFn = parse2(item2.clone()).unwrap();
    let name = item_parsed.sig.ident;

    let expanded = quote! {
        use conjure_core::rule::Rule;
        use conjure_core::rule::RuleKind;

        #item2

        println!("{:?}", Rule {
            name: String::from(stringify!(#name)),
            kind: RuleKind::#args2,
            application: #name,
        });
    };

    proc_macro::TokenStream::from(expanded)
}

#[proc_macro_attribute]
pub fn show_streams(
    attr: proc_macro::TokenStream,
    item: proc_macro::TokenStream,
) -> proc_macro::TokenStream {
    println!("attr: \"{}\"", attr);
    println!("item: \"{}\"", item);
    item
}
