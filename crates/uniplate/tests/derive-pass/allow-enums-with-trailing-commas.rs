use uniplate::Uniplate;

#[derive(PartialEq,Eq,Clone,Uniplate)]
enum NoTrailingComma {
    B(Vec<NoTrailingComma>)
}

#[derive(PartialEq,Eq,Clone,Uniplate)]
enum TrailingComma {
    B(Vec<TrailingComma>),
}

pub fn main() {
    
}

