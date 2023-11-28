use quote::quote;

use syn::{parse2, ItemFn};

// #[rule(Horizontal)]
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

        #item2
        //inventory::submit! {
        //    Rule {
        //        name: "#name",
        //        kind: #args2,
        //        application: #name,
        //    }
        //}

        //inventory::collect!(Rule);

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
