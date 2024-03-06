use uniplate_derive::Uniplate;
use uniplate::uniplate::Uniplate;

#[test]
fn derive_children() {
    #[derive(Clone, PartialEq, Eq, Uniplate)]
    enum TestEnum {
        A(i32),
        B(bool),
    }


}