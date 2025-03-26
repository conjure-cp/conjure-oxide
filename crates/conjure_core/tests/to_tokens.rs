use conjure_core::ast::{Atom, Expression};
use conjure_core::metadata::Metadata;
use derive_to_tokens::ToTokens;
use quote::{quote, ToTokens};

#[derive(ToTokens, Clone)]
enum TestEnum {
    A,
    B(String),
    C(i32, bool),
    D(#[to_tokens(recursive)] Box<TestEnum>),
    E(#[to_tokens(recursive)] Vec<TestEnum>),
    F(#[to_tokens(recursive)] Option<Box<TestEnum>>),
    G(Vec<Vec<i32>>),
}

#[derive(ToTokens, Clone)]
struct TestStruct {
    a: i32,
    b: String,
    c: TestEnum,
    #[to_tokens(recursive)]
    d: Option<Box<TestStruct>>,
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

fn assert_tokens_eq<A: ToTokens, B: ToTokens>(a: &A, b: &B) {
    let ts1 = a.to_token_stream().to_string();
    let ts2 = b.to_token_stream().to_string();
    assert_eq!(ts1, ts2);
}

#[test]
pub fn test_enum() {
    let a = TestEnum::A;
    let b = TestEnum::B("Hello".into());
    let c = TestEnum::C(42, true);
    let d = TestEnum::D(Box::new(TestEnum::A));
    let e = TestEnum::E(vec![TestEnum::A, TestEnum::B("World".into())]);
    let f1 = TestEnum::F(Some(Box::new(TestEnum::A.into())));
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

    assert_tokens_eq(&a, &quote! { TestEnum::A });
    assert_tokens_eq(&b, &quote! { TestEnum::B("Hello".into()) });
    assert_tokens_eq(&c, &quote! { TestEnum::C(42i32.into(), true.into()) });
    assert_tokens_eq(&d, &quote! { TestEnum::D(Box::new(TestEnum::A.into())) });
    assert_tokens_eq(
        &e,
        &quote! { TestEnum::E(vec![TestEnum::A.into(), TestEnum::B("World".into()).into()]) },
    );
    assert_tokens_eq(
        &f1,
        &quote! { TestEnum::F(Some(Box::new(TestEnum::A.into()))) },
    );
    assert_tokens_eq(&f2, &quote! { TestEnum::F(None) });
    assert_tokens_eq(
        &g,
        &quote! {
            TestEnum::G(
                vec![
                    vec![1i32.into(), 2i32.into(), 3i32.into()],
                    vec![4i32.into(), 5i32.into(), 6i32.into()]
                ]
            )
        },
    );
}

#[test]
pub fn test_struct() {
    let leaf = TestStruct {
        a: 42,
        b: "Hello".to_string(),
        c: TestEnum::D(Box::new(TestEnum::A)),
        d: None,
    };
    let tree = TestStruct {
        a: 52,
        b: "world".to_string(),
        c: TestEnum::G(vec![vec![1, 2, 3], vec![4, 5, 6]]),
        d: Some(Box::new(leaf.clone())),
    };
    println!("{}", quote! {#leaf});
    println!("{}", quote! {#tree});
}

#[test]
pub fn test_simple_expression() {
    let expr = Expression::Eq(
        Metadata::new(),
        Box::new(Expression::Atomic(Metadata::new(), Atom::new_uref("x"))),
        Box::new(Expression::Sum(
            Metadata::new(),
            vec![
                Expression::Atomic(Metadata::new(), Atom::new_uref("y")),
                Expression::Atomic(Metadata::new(), Atom::new_ilit(42)),
            ],
        )),
    );

    println!("{}", quote! {#expr});
}
