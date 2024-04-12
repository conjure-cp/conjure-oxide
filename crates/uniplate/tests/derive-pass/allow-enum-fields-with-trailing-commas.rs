use uniplate::Uniplate;

#[derive(Uniplate,PartialEq,Eq,Clone)]
enum NoTrailingComma {
    B(Vec<NoTrailingComma>)
}

#[derive(Uniplate,PartialEq,Eq,Clone)]
enum TrailingCommaInField {
    B(Vec<TrailingCommaInField>,)
}

pub fn main() {
    
}
