use uniplate::Uniplate;
#[derive(PartialEq,Eq,Clone,Uniplate)]
enum MyEnum {
    A(Vec<Vec<MyEnum>>),
    B(Vec<Vec<Vec<MyEnum>>>)
}

pub fn main() {}
