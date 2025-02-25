//Simple program to generate random addmul expressions.

use rand::{self, Rng};

#[derive(Debug)]
enum Expr {
    Add(Box<Expr>, Box<Expr>),
    Mul(Box<Expr>, Box<Expr>),
    Val(i32),
}

fn random_exp_tree(depth: usize) -> Expr {
    let mut rng = rand::rng();

    if depth == 0 {
        return Expr::Val(rng.random_range(1..10));
    }

    match rng.random_range(1..=10) {
        x if (1..=4).contains(&x) => Expr::Add(
            Box::new(random_exp_tree(depth - 1)),
            Box::new(random_exp_tree(depth - 1)),
        ),
        x if (5..=8).contains(&x) => Expr::Mul(
            Box::new(random_exp_tree(depth - 1)),
            Box::new(random_exp_tree(depth - 1)),
        ),
        _ => Expr::Val(rng.random_range(1..=10)),
    }
}

fn expr_to_string(expr: &Expr) -> String {
    match expr {
        Expr::Val(n) => n.to_string(),
        Expr::Add(a, b) => format!("({} + {})", expr_to_string(a), expr_to_string(b)),
        Expr::Mul(a, b) => format!("({} * {})", expr_to_string(a), expr_to_string(b)),
    }
}

/*
fn main() {
    let expr = random_exp_tree(5);
    println!("Random Expression Tree is: {}", expr_to_string(&expr));
}
*/
