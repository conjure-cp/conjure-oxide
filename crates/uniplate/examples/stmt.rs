use uniplate::unreachable;
use uniplate::Uniplate;

unreachable!(Expr, Stmt);
unreachable!(String, Expr);
unreachable!(String, Stmt);
unreachable!(Expr, String);
unreachable!(i32, Expr);

#[derive(Eq, PartialEq, Clone, Debug, Uniplate)]
enum Stmt {
    Assign(String, Expr),
    //Sequence(Vec<Stmt>),
    If(Expr, Box<Stmt>, Box<Stmt>),
    While(Expr, Box<Stmt>),
}

#[derive(Eq, PartialEq, Clone, Debug, Uniplate)]
enum Expr {
    Add(Box<Expr>, Box<Expr>),
    Sub(Box<Expr>, Box<Expr>),
    Mul(Box<Expr>, Box<Expr>),
    Div(Box<Expr>, Box<Expr>),
    //Val(i32),
    //Var(String),
    Neg(Box<Expr>),
}

pub fn main() {}
