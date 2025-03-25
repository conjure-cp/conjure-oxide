use derive_to_tokens::ToTokens;
use quote::quote;

#[derive(ToTokens)]
enum TestEnum {
    A,
    B(String),
    C(i32, bool),
    D(#[to_tokens(recursive)] Box<TestEnum>),
    E(#[to_tokens(recursive)] Vec<TestEnum>),
}

#[test]
pub fn test_enum() {
    let a = TestEnum::A;
    let b = TestEnum::B("Hello".to_string());
    let c = TestEnum::C(42, true);
    let d = TestEnum::D(Box::new(TestEnum::A));

    println!("{}", quote! {#a});
    println!("{}", quote! {#b});
    println!("{}", quote! {#c});
    println!("{}", quote! {#d});
}
