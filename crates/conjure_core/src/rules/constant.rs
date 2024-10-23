use conjure_core::ast::{Constant as Const, Expression as Expr};
use conjure_core::metadata::Metadata;
use conjure_core::rule_engine::{
    register_rule, register_rule_set, ApplicationError, ApplicationResult, Reduction,
};
use conjure_core::Model;

register_rule_set!("Constant", 100, ());

#[register_rule(("Constant", 9001))]
fn apply_eval_constant(expr: &Expr, _: &Model) -> ApplicationResult {
    if let Expr::Constant(_, _) = expr {
        return Err(ApplicationError::RuleNotApplicable);
    }
    eval_constant(expr)
        .map(|c| Reduction::pure(Expr::Constant(Metadata::new(), c)))
        .ok_or(ApplicationError::RuleNotApplicable)
}

/// Simplify an expression to a constant if possible
/// Returns:
/// `None` if the expression cannot be simplified to a constant (e.g. if it contains a variable)
/// `Some(Const)` if the expression can be simplified to a constant
pub fn eval_constant(expr: &Expr) -> Option<Const> {
    match expr {
        Expr::Constant(_, c) => Some(c.clone()),
        Expr::Reference(_, _) => None,
        Expr::Eq(_, a, b) => bin_op::<i32, bool>(|a, b| a == b, a, b)
            .or_else(|| bin_op::<bool, bool>(|a, b| a == b, a, b))
            .map(Const::Bool),
        Expr::Neq(_, a, b) => bin_op::<i32, bool>(|a, b| a != b, a, b).map(Const::Bool),
        Expr::Lt(_, a, b) => bin_op::<i32, bool>(|a, b| a < b, a, b).map(Const::Bool),
        Expr::Gt(_, a, b) => bin_op::<i32, bool>(|a, b| a > b, a, b).map(Const::Bool),
        Expr::Leq(_, a, b) => bin_op::<i32, bool>(|a, b| a <= b, a, b).map(Const::Bool),
        Expr::Geq(_, a, b) => bin_op::<i32, bool>(|a, b| a >= b, a, b).map(Const::Bool),

        Expr::Not(_, expr) => un_op::<bool, bool>(|e| !e, expr).map(Const::Bool),

        Expr::And(_, exprs) => {
            vec_op::<bool, bool>(|e| e.iter().all(|&e| e), exprs).map(Const::Bool)
        }
        Expr::Or(_, exprs) => {
            vec_op::<bool, bool>(|e| e.iter().any(|&e| e), exprs).map(Const::Bool)
        }

        Expr::Sum(_, exprs) => vec_op::<i32, i32>(|e| e.iter().sum(), exprs).map(Const::Int),

        Expr::Ineq(_, a, b, c) => {
            tern_op::<i32, bool>(|a, b, c| a <= (b + c), a, b, c).map(Const::Bool)
        }

        Expr::SumGeq(_, exprs, a) => {
            flat_op::<i32, bool>(|e, a| e.iter().sum::<i32>() >= a, exprs, a).map(Const::Bool)
        }
        Expr::SumLeq(_, exprs, a) => {
            flat_op::<i32, bool>(|e, a| e.iter().sum::<i32>() <= a, exprs, a).map(Const::Bool)
        }
        // Expr::Div(_, a, b) => bin_op::<i32, i32>(|a, b| a / b, a, b).map(Const::Int),
        // Expr::SafeDiv(_, a, b) => bin_op::<i32, i32>(|a, b| a / b, a, b).map(Const::Int),
        Expr::Min(_, exprs) => {
            opt_vec_op::<i32, i32>(|e| e.iter().min().copied(), exprs).map(Const::Int)
        }
        Expr::Max(_, exprs) => {
            opt_vec_op::<i32, i32>(|e| e.iter().max().copied(), exprs).map(Const::Int)
        }
        Expr::UnsafeDiv(_, a, b) | Expr::SafeDiv(_, a, b) => {
            if unwrap_expr::<i32>(b)? == 0 {
                return None;
            }
            bin_op::<i32, i32>(|a, b| a / b, a, b).map(Const::Int)
        }
        Expr::DivEq(_, a, b, c) => {
            tern_op::<i32, bool>(|a, b, c| a == b * c, a, b, c).map(Const::Bool)
        }
        Expr::Bubble(_, a, b) => bin_op::<bool, bool>(|a, b| a && b, a, b).map(Const::Bool),

        Expr::Reify(_, a, b) => bin_op::<bool, bool>(
            |a, b| {
                if a {
                    a && b
                } else {
                    !a && !b
                }
            },
            a,
            b,
        )
        .map(Const::Bool),
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

fn vec_op<T, A>(f: fn(Vec<T>) -> A, a: &[Expr]) -> Option<A>
where
    T: TryFrom<Const>,
{
    let a = a.iter().map(unwrap_expr).collect::<Option<Vec<T>>>()?;
    Some(f(a))
}

fn opt_vec_op<T, A>(f: fn(Vec<T>) -> Option<A>, a: &[Expr]) -> Option<A>
where
    T: TryFrom<Const>,
{
    let a = a.iter().map(unwrap_expr).collect::<Option<Vec<T>>>()?;
    f(a)
}

fn flat_op<T, A>(f: fn(Vec<T>, T) -> A, a: &[Expr], b: &Expr) -> Option<A>
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

#[cfg(test)]
mod tests {
    use conjure_core::ast::{Constant, Expression};

    #[test]
    fn div_by_zero() {
        let expr = Expression::UnsafeDiv(
            Default::default(),
            Box::new(Expression::Constant(Default::default(), Constant::Int(1))),
            Box::new(Expression::Constant(Default::default(), Constant::Int(0))),
        );
        assert_eq!(super::eval_constant(&expr), None);
    }

    #[test]
    fn safediv_by_zero() {
        let expr = Expression::SafeDiv(
            Default::default(),
            Box::new(Expression::Constant(Default::default(), Constant::Int(1))),
            Box::new(Expression::Constant(Default::default(), Constant::Int(0))),
        );
        assert_eq!(super::eval_constant(&expr), None);
    }
}
