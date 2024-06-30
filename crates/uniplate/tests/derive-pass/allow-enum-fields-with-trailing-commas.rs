use uniplate::Uniplate;

#[derive(Uniplate,PartialEq,Eq,Clone)]
#[uniplate()]
enum NoTrailingComma {
    B(Vec<NoTrailingComma>)
}

#[derive(Uniplate,PartialEq,Eq,Clone)]
#[uniplate()]
enum TrailingCommaInField {
    B(Vec<TrailingCommaInField>,)
}

pub fn main() {
    
}
