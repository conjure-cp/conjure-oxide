use conjure_core::ast::{Atom, Expression as Expr, Literal as Lit};
use conjure_core::metadata::Metadata;
use conjure_core::rule_engine::{
    register_rule, register_rule_set, ApplicationError, ApplicationResult, Reduction,
};
use conjure_core::Model;
use tracing::warn;

register_rule_set!("Constant", 100, ());

#[register_rule(("Constant", 9001))]
fn apply_eval_constant(expr: &Expr, _: &Model) -> ApplicationResult {
    if let Expr::Atomic(_, Atom::Literal(_)) = expr {
        return Err(ApplicationError::RuleNotApplicable);
    }
    eval_constant(expr)
        .map(|c| Reduction::pure(Expr::Atomic(Metadata::new(), Atom::Literal(c))))
        .ok_or(ApplicationError::RuleNotApplicable)
}

/// Simplify an expression to a constant if possible
/// Returns:
/// `None` if the expression cannot be simplified to a constant (e.g. if it contains a variable)
/// `Some(Const)` if the expression can be simplified to a constant
pub fn eval_constant(expr: &Expr) -> Option<Lit> {
    match expr {
        Expr::Atomic(_, Atom::Literal(c)) => Some(c.clone()),
        Expr::Atomic(_, Atom::Reference(_c)) => None,
        Expr::Eq(_, a, b) => bin_op::<i32, bool>(|a, b| a == b, a, b)
            .or_else(|| bin_op::<bool, bool>(|a, b| a == b, a, b))
            .map(Lit::Bool),
        Expr::Neq(_, a, b) => bin_op::<i32, bool>(|a, b| a != b, a, b).map(Lit::Bool),
        Expr::Lt(_, a, b) => bin_op::<i32, bool>(|a, b| a < b, a, b).map(Lit::Bool),
        Expr::Gt(_, a, b) => bin_op::<i32, bool>(|a, b| a > b, a, b).map(Lit::Bool),
        Expr::Leq(_, a, b) => bin_op::<i32, bool>(|a, b| a <= b, a, b).map(Lit::Bool),
        Expr::Geq(_, a, b) => bin_op::<i32, bool>(|a, b| a >= b, a, b).map(Lit::Bool),

        Expr::Not(_, expr) => un_op::<bool, bool>(|e| !e, expr).map(Lit::Bool),

        Expr::And(_, exprs) => vec_op::<bool, bool>(|e| e.iter().all(|&e| e), exprs).map(Lit::Bool),
        Expr::Or(_, exprs) => vec_op::<bool, bool>(|e| e.iter().any(|&e| e), exprs).map(Lit::Bool),

        Expr::Sum(_, exprs) => vec_op::<i32, i32>(|e| e.iter().sum(), exprs).map(Lit::Int),

        Expr::Ineq(_, a, b, c) => {
            tern_op::<i32, bool>(|a, b, c| a <= (b + c), a, b, c).map(Lit::Bool)
        }

        Expr::SumGeq(_, exprs, a) => {
            flat_op::<i32, bool>(|e, a| e.iter().sum::<i32>() >= a, exprs, a).map(Lit::Bool)
        }
        Expr::SumLeq(_, exprs, a) => {
            flat_op::<i32, bool>(|e, a| e.iter().sum::<i32>() <= a, exprs, a).map(Lit::Bool)
        }
        // Expr::Div(_, a, b) => bin_op::<i32, i32>(|a, b| a / b, a, b).map(Lit::Int),
        // Expr::SafeDiv(_, a, b) => bin_op::<i32, i32>(|a, b| a / b, a, b).map(Lit::Int),
        Expr::Min(_, exprs) => {
            opt_vec_op::<i32, i32>(|e| e.iter().min().copied(), exprs).map(Lit::Int)
        }
        Expr::Max(_, exprs) => {
            opt_vec_op::<i32, i32>(|e| e.iter().max().copied(), exprs).map(Lit::Int)
        }
        Expr::UnsafeDiv(_, a, b) | Expr::SafeDiv(_, a, b) => {
            if unwrap_expr::<i32>(b)? == 0 {
                return None;
            }
            bin_op::<i32, i32>(|a, b| a / b, a, b).map(Lit::Int)
        }
        Expr::UnsafeMod(_, a, b) | Expr::SafeMod(_, a, b) => {
            if unwrap_expr::<i32>(b)? == 0 {
                return None;
            }
            bin_op::<i32, i32>(|a, b| a % b, a, b).map(Lit::Int)
        }
        Expr::DivEqUndefZero(_, a, b, c) => {
            let a = unwrap_atom::<i32>(a)?;
            let b = unwrap_atom::<i32>(b)?;
            let c = unwrap_atom::<i32>(c)?;

            if b == 0 {
                return None;
            }

            Some(Lit::Bool(a / b == c))
        }
        Expr::Bubble(_, a, b) => bin_op::<bool, bool>(|a, b| a && b, a, b).map(Lit::Bool),

        Expr::Reify(_, a, b) => bin_op::<bool, bool>(|a, b| a == b, a, b).map(Lit::Bool),
        _ => {
            warn!(%expr,"Unimplemented constant eval");
            None
        }
    }
}

fn un_op<T, A>(f: fn(T) -> A, a: &Expr) -> Option<A>
where
    T: TryFrom<Lit>,
{
    let a = unwrap_expr::<T>(a)?;
    Some(f(a))
}

fn bin_op<T, A>(f: fn(T, T) -> A, a: &Expr, b: &Expr) -> Option<A>
where
    T: TryFrom<Lit>,
{
    let a = unwrap_expr::<T>(a)?;
    let b = unwrap_expr::<T>(b)?;
    Some(f(a, b))
}

fn tern_op<T, A>(f: fn(T, T, T) -> A, a: &Expr, b: &Expr, c: &Expr) -> Option<A>
where
    T: TryFrom<Lit>,
{
    let a = unwrap_expr::<T>(a)?;
    let b = unwrap_expr::<T>(b)?;
    let c = unwrap_expr::<T>(c)?;
    Some(f(a, b, c))
}

fn vec_op<T, A>(f: fn(Vec<T>) -> A, a: &[Expr]) -> Option<A>
where
    T: TryFrom<Lit>,
{
    let a = a.iter().map(unwrap_expr).collect::<Option<Vec<T>>>()?;
    Some(f(a))
}

fn opt_vec_op<T, A>(f: fn(Vec<T>) -> Option<A>, a: &[Expr]) -> Option<A>
where
    T: TryFrom<Lit>,
{
    let a = a.iter().map(unwrap_expr).collect::<Option<Vec<T>>>()?;
    f(a)
}

fn flat_op<T, A>(f: fn(Vec<T>, T) -> A, a: &[Expr], b: &Expr) -> Option<A>
where
    T: TryFrom<Lit>,
{
    let a = a.iter().map(unwrap_expr).collect::<Option<Vec<T>>>()?;
    let b = unwrap_expr::<T>(b)?;
    Some(f(a, b))
}

fn unwrap_expr<T: TryFrom<Lit>>(expr: &Expr) -> Option<T> {
    let c = eval_constant(expr)?;
    TryInto::<T>::try_into(c).ok()
}

fn unwrap_atom<T: TryFrom<Lit>>(atom: &Atom) -> Option<T> {
    let Atom::Literal(c) = atom else {
        return None;
    };
    TryInto::<T>::try_into(c.clone()).ok()
}

#[cfg(test)]
mod tests {
    use conjure_core::ast::{Atom, Expression, Literal};

    #[test]
    fn div_by_zero() {
        let expr = Expression::UnsafeDiv(
            Default::default(),
            Box::new(Expression::Atomic(
                Default::default(),
                Atom::Literal(Literal::Int(1)),
            )),
            Box::new(Expression::Atomic(
                Default::default(),
                Atom::Literal(Literal::Int(0)),
            )),
        );
        assert_eq!(super::eval_constant(&expr), None);
    }

    #[test]
    fn safediv_by_zero() {
        let expr = Expression::SafeDiv(
            Default::default(),
            Box::new(Expression::Atomic(
                Default::default(),
                Atom::Literal(Literal::Int(1)),
            )),
            Box::new(Expression::Atomic(
                Default::default(),
                Atom::Literal(Literal::Int(0)),
            )),
        );
        assert_eq!(super::eval_constant(&expr), None);
    }
}
