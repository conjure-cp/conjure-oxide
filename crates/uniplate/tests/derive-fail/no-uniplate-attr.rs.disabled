use uniplate::Uniplate;
#[derive(Eq, PartialEq, Clone, Debug, Uniplate)]
enum Stmt {
    Assign(String, Expr),
    //Sequence(Vec<Stmt>),
    If(Expr, Box<Stmt>, Box<Stmt>),
    While(Expr, Box<Stmt>),
}

#[derive(Eq, PartialEq, Clone, Debug, Uniplate)]
#[uniplate()]
enum Expr {}

pub fn main() {
    
}

