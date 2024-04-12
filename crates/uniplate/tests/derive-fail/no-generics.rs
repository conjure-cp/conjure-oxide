use uniplate::Uniplate;

#[derive(PartialEq,Eq,Clone,Uniplate)]
enum MyEnum {
    A(F<i32>,G)
}

#[derive(PartialEq,Eq,Clone)]
struct F<T: PartialEq + Eq + Clone> {
    _data: std::marker::PhantomData<T>
}

#[derive(PartialEq,Eq,Clone)]
struct G {}

pub fn main() {
    
}
