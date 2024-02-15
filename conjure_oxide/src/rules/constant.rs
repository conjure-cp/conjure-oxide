use conjure_core::{ast::Constant as Const, ast::Expression as Expr, rule::RuleApplicationError};
use conjure_rules::register_rule;

#[register_rule]
fn apply_eval_constant(expr: &Expr) -> Result<Expr, RuleApplicationError> {
    if expr.is_constant() {
        return Err(RuleApplicationError::RuleNotApplicable);
    }

    eval_constant(expr)
        .map(Expr::Constant)
        .ok_or(RuleApplicationError::RuleNotApplicable)
}

/// Simplify an expression to a constant if possible
/// Returns:
/// `None` if the expression cannot be simplified to a constant (e.g. if it contains a variable)
/// `Some(Const)` if the expression can be simplified to a constant
pub fn eval_constant(expr: &Expr) -> Option<Const> {
    match expr {
        Expr::Constant(c) => Some(c.clone()),
        Expr::Reference(_) => None,

        Expr::Eq(a, b) => bin_op::<i32, bool>(|a, b| a == b, a, b)
            .or_else(|| bin_op::<bool, bool>(|a, b| a == b, a, b))
            .map(Const::Bool),
        Expr::Neq(a, b) => bin_op::<i32, bool>(|a, b| a != b, a, b).map(Const::Bool),
        Expr::Lt(a, b) => bin_op::<i32, bool>(|a, b| a < b, a, b).map(Const::Bool),
        Expr::Gt(a, b) => bin_op::<i32, bool>(|a, b| a > b, a, b).map(Const::Bool),
        Expr::Leq(a, b) => bin_op::<i32, bool>(|a, b| a <= b, a, b).map(Const::Bool),
        Expr::Geq(a, b) => bin_op::<i32, bool>(|a, b| a >= b, a, b).map(Const::Bool),

        Expr::Not(expr) => un_op::<bool, bool>(|e| !e, expr).map(Const::Bool),

        Expr::And(exprs) => vec_op::<bool, bool>(|e| e.iter().all(|&e| e), exprs).map(Const::Bool),
        Expr::Or(exprs) => vec_op::<bool, bool>(|e| e.iter().any(|&e| e), exprs).map(Const::Bool),

        Expr::Sum(exprs) => vec_op::<i32, i32>(|e| e.iter().sum(), exprs).map(Const::Int),

        Expr::Ineq(a, b, c) => {
            tern_op::<i32, bool>(|a, b, c| a <= (b + c), a, b, c).map(Const::Bool)
        }

        Expr::SumGeq(exprs, a) => {
            flat_op::<i32, bool>(|e, a| e.iter().sum::<i32>() >= a, exprs, a).map(Const::Bool)
        }
        Expr::SumLeq(exprs, a) => {
            flat_op::<i32, bool>(|e, a| e.iter().sum::<i32>() <= a, exprs, a).map(Const::Bool)
        }
        _ => {
            println!("WARNING: Unimplemented constant eval: {:?}", expr);
            None
        }
    }
}

fn un_op<T, A>(f: fn(T) -> A, a: &Expr) -> Option<A>
where
    T: TryFrom<Const>,
{
    let a = unwrap_expr::<T>(a)?;
    Some(f(a))
}

fn bin_op<T, A>(f: fn(T, T) -> A, a: &Expr, b: &Expr) -> Option<A>
where
    T: TryFrom<Const>,
{
    let a = unwrap_expr::<T>(a)?;
    let b = unwrap_expr::<T>(b)?;
    Some(f(a, b))
}

fn tern_op<T, A>(f: fn(T, T, T) -> A, a: &Expr, b: &Expr, c: &Expr) -> Option<A>
where
    T: TryFrom<Const>,
{
    let a = unwrap_expr::<T>(a)?;
    let b = unwrap_expr::<T>(b)?;
    let c = unwrap_expr::<T>(c)?;
    Some(f(a, b, c))
}

fn vec_op<T, A>(f: fn(Vec<T>) -> A, a: &Vec<Expr>) -> Option<A>
where
    T: TryFrom<Const>,
{
    let a = a.iter().map(unwrap_expr).collect::<Option<Vec<T>>>()?;
    Some(f(a))
}

fn flat_op<T, A>(f: fn(Vec<T>, T) -> A, a: &Vec<Expr>, b: &Expr) -> Option<A>
where
    T: TryFrom<Const>,
{
    let a = a.iter().map(unwrap_expr).collect::<Option<Vec<T>>>()?;
    let b = unwrap_expr::<T>(b)?;
    Some(f(a, b))
}

fn unwrap_expr<T: TryFrom<Const>>(expr: &Expr) -> Option<T> {
    let c = eval_constant(expr)?;
    TryInto::<T>::try_into(c).ok()
}
