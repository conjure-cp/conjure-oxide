use uniplate::uniplate::Uniplate;
use uniplate_derive::Uniplate;

#[derive(Clone, Debug, PartialEq, Eq, Uniplate)]
enum TestEnum {
    A(i32),
    B(Box<TestEnum>),
    C(Vec<TestEnum>),
    D(bool, Box<TestEnum>),
    E(Box<TestEnum>, Box<TestEnum>),
    F((Box<TestEnum>, Box<TestEnum>)),
    G((Box<TestEnum>, (Box<TestEnum>, i32))),
    H(Vec<Vec<TestEnum>>),
}

#[test]
fn derive_context_empty() {
    let a = TestEnum::A(42);
    let context = a.uniplate().1;
    assert_eq!(context(vec![]), a)
}

#[test]
fn derive_context_box() {
    let a = TestEnum::A(42);
    let b = TestEnum::B(Box::new(a.clone()));
    let context = b.uniplate().1;
    assert_eq!(context(vec![a.clone()]), b);
}

#[test]
fn derive_children_empty() {
    let a = TestEnum::A(42);
    let children = a.uniplate().0;
    assert_eq!(children, vec![]);
}

#[test]
fn derive_children_box() {
    let b = TestEnum::B(Box::new(TestEnum::A(42)));
    let children = b.uniplate().0;
    assert_eq!(children, vec![TestEnum::A(42)]);
}

#[test]
fn derive_children_vec() {
    let c = TestEnum::C(vec![TestEnum::A(1), TestEnum::B(Box::new(TestEnum::A(2)))]);
    let children = c.uniplate().0;
    assert_eq!(
        children,
        vec![TestEnum::A(1), TestEnum::B(Box::new(TestEnum::A(2))),]
    );
}

#[test]
fn derive_children_two() {
    let d = TestEnum::D(true, Box::new(TestEnum::A(42)));
    let children = d.uniplate().0;
    assert_eq!(children, vec![TestEnum::A(42)]);
}

#[test]
fn derive_children_tuple() {
    let e = TestEnum::F((Box::new(TestEnum::A(1)), Box::new(TestEnum::A(2))));
    let children = e.uniplate().0;
    assert_eq!(children, vec![TestEnum::A(1), TestEnum::A(2),]);
}

#[test]
fn derive_children_different_variants() {
    let f = TestEnum::E(
        Box::new(TestEnum::A(1)),
        Box::new(TestEnum::B(Box::new(TestEnum::A(2)))),
    );
    let children = f.uniplate().0;
    assert_eq!(
        children,
        vec![TestEnum::A(1), TestEnum::B(Box::new(TestEnum::A(2)))]
    );
}

#[test]
fn derive_children_nested_tuples() {
    let g = TestEnum::G((Box::new(TestEnum::A(1)), (Box::new(TestEnum::A(2)), 42)));
    let children = g.uniplate().0;
    assert_eq!(children, vec![TestEnum::A(1), TestEnum::A(2)])
}

#[test]
fn derive_children_nested_vectors() {
    let h = TestEnum::H(vec![
        vec![TestEnum::A(1), TestEnum::A(2)],
        vec![TestEnum::A(3), TestEnum::A(4)],
    ]);
    let children = h.uniplate().0;
    assert_eq!(
        children,
        vec![
            TestEnum::A(1),
            TestEnum::A(2),
            TestEnum::A(3),
            TestEnum::A(4)
        ]
    )
}
