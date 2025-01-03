use std::collections::HashSet;

use conjure_core::ast::{Atom, Expression as Expr, Literal as Lit};
use conjure_core::metadata::Metadata;
use conjure_core::rule_engine::{
    register_rule, register_rule_set, ApplicationError, ApplicationResult, Reduction,
};
use conjure_core::Model;

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
        Expr::Product(_, exprs) => vec_op::<i32, i32>(|e| e.iter().product(), exprs).map(Lit::Int),

        Expr::FlatIneq(_, a, b, c) => {
            let a: i32 = a.try_into().ok()?;
            let b: i32 = b.try_into().ok()?;
            let c: i32 = c.try_into().ok()?;

            Some(Lit::Bool(a <= b + c))
        }

        Expr::FlatSumGeq(_, exprs, a) => {
            let sum = exprs.iter().try_fold(0, |acc, atom: &Atom| {
                let n: i32 = atom.try_into().ok()?;
                let acc = acc + n;
                Some(acc)
            })?;

            Some(Lit::Bool(sum >= a.try_into().ok()?))
        }
        Expr::FlatSumLeq(_, exprs, a) => {
            let sum = exprs.iter().try_fold(0, |acc, atom: &Atom| {
                let n: i32 = atom.try_into().ok()?;
                let acc = acc + n;
                Some(acc)
            })?;

            Some(Lit::Bool(sum >= a.try_into().ok()?))
        }
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
            bin_op::<i32, i32>(|a, b| ((a as f32) / (b as f32)).floor() as i32, a, b).map(Lit::Int)
        }
        Expr::UnsafeMod(_, a, b) | Expr::SafeMod(_, a, b) => {
            if unwrap_expr::<i32>(b)? == 0 {
                return None;
            }
            bin_op::<i32, i32>(|a, b| a - b * (a as f32 / b as f32).floor() as i32, a, b)
                .map(Lit::Int)
        }
        Expr::MinionDivEqUndefZero(_, a, b, c) => {
            // div always rounds down
            let a: i32 = a.try_into().ok()?;
            let b: i32 = b.try_into().ok()?;
            let c: i32 = c.try_into().ok()?;

            if b == 0 {
                return None;
            }

            let a = a as f32;
            let b = b as f32;
            let div: i32 = (a / b).floor() as i32;
            Some(Lit::Bool(div == c))
        }
        Expr::Bubble(_, a, b) => bin_op::<bool, bool>(|a, b| a && b, a, b).map(Lit::Bool),

        Expr::MinionReify(_, a, b) => {
            let result = eval_constant(a)?;

            let result: bool = result.try_into().ok()?;
            let b: bool = b.try_into().ok()?;

            Some(Lit::Bool(b == result))
        }
        Expr::SumEq(_, exprs, a) => {
            flat_op::<i32, bool>(|e, a| e.iter().sum::<i32>() == a, exprs, a).map(Lit::Bool)
        }
        Expr::MinionModuloEqUndefZero(_, a, b, c) => {
            // From Savile Row. Same semantics as division.
            //
            //   a - (b * floor(a/b))
            //
            // We don't use % as it has the same semantics as /. We don't use / as we want to round
            // down instead, not towards zero.

            let a: i32 = a.try_into().ok()?;
            let b: i32 = b.try_into().ok()?;
            let c: i32 = c.try_into().ok()?;

            if b == 0 {
                return None;
            }

            let modulo = a - b * (a as f32 / b as f32).floor() as i32;
            Some(Lit::Bool(modulo == c))
        }
        Expr::AllDiff(_, es) => {
            let mut lits: HashSet<Lit> = HashSet::new();
            for expr in es {
                let Expr::Atomic(_, Atom::Literal(x)) = expr else {
                    return None;
                };
                if lits.contains(x) {
                    return Some(Lit::Bool(false));
                } else {
                    lits.insert(x.clone());
                }
            }
            Some(Lit::Bool(true))
        }
        Expr::FlatWatchedLiteral(_, _, _) => None,
        Expr::AuxDeclaration(_, _, _) => None,
        Expr::Neg(_, a) => {
            let a: &Atom = a.try_into().ok()?;
            let a: i32 = a.try_into().ok()?;
            Some(Lit::Int(-a))
        }
        Expr::Minus(_, a, b) => {
            let a: &Atom = a.try_into().ok()?;
            let a: i32 = a.try_into().ok()?;

            let b: &Atom = b.try_into().ok()?;
            let b: i32 = b.try_into().ok()?;

            Some(Lit::Int(a - b))
        }
        Expr::FlatMinusEq(_, a, b) => {
            let a: i32 = a.try_into().ok()?;
            let b: i32 = b.try_into().ok()?;
            Some(Lit::Bool(a == -b))
        } // _ => {
          //     warn!(%expr,"Unimplemented constant eval");
          //     None
          // }
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

#[allow(dead_code)]
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

#[cfg(test)]
mod tests {
    use crate::rules::eval_constant;
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
        assert_eq!(eval_constant(&expr), None);
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
        assert_eq!(eval_constant(&expr), None);
    }
}
