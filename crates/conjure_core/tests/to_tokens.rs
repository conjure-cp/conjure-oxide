use derive_to_tokens::ToTokens;
use quote::quote;

#[derive(ToTokens)]
enum TestEnum {
    A,
    B(String),
    C(i32, bool),
    D(#[to_tokens(recursive)] Box<TestEnum>),
    E(#[to_tokens(recursive)] Vec<TestEnum>),
    F(#[to_tokens(recursive)] Option<Box<TestEnum>>),
    G(Vec<Vec<i32>>)
}

// enum TestEnum {
//     A,
//     B(String),
//     C(i32, bool),
//     D(Box<TestEnum>),
//     E(Vec<TestEnum>),
//     F(Option<Box<TestEnum>>),
//     G(Vec<Vec<i32>>)
// }

#[test]
pub fn test_enum() {
    let a = TestEnum::A;
    let b = TestEnum::B("Hello".to_string());
    let c = TestEnum::C(42, true);
    let d = TestEnum::D(Box::new(TestEnum::A));
    let e = TestEnum::E(vec![TestEnum::A, TestEnum::B("World".to_string())]);
    let f1 = TestEnum::F(Some(Box::new(TestEnum::A)));
    let f2 = TestEnum::F(None);
    let g = TestEnum::G(vec![vec![1, 2, 3], vec![4, 5, 6]]);

    println!("{}", quote! {#a});
    println!("{}", quote! {#b});
    println!("{}", quote! {#c});
    println!("{}", quote! {#d});
    println!("{}", quote! {#e});
    println!("{}", quote! {#f1});
    println!("{}", quote! {#f2});
    println!("{}", quote! {#g});
}
