use uniplate_derive::Uniplate;
use uniplate::uniplate::Uniplate;

#[test]
fn derive_children() {
    #[derive(Clone, PartialEq, Eq, Uniplate)]
    enum TestEnum {
        A(i32),
        B(Box<TestEnum>),
        C(Vec<TestEnum>),
        // D([Box<TestEnum>; 2]),
        // E((Box<TestEnum>, Box<TestEnum>)),
        F((Box<TestEnum>, i32)),
        I,
    }
}