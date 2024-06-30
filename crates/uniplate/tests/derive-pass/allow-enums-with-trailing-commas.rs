use uniplate::Uniplate;

#[derive(PartialEq,Eq,Clone,Uniplate)]
#[uniplate()]
enum NoTrailingComma {
    B(Vec<NoTrailingComma>)
}

#[derive(PartialEq,Eq,Clone,Uniplate)]
#[uniplate()]
enum TrailingComma {
    B(Vec<TrailingComma>),
}

pub fn main() {
    
}

