use std::collections::HashSet;

use conjure_core::ast::{Atom, Expression as Expr, Literal as Lit};
use conjure_core::metadata::Metadata;
use conjure_core::rule_engine::{
    register_rule, register_rule_set, ApplicationError, ApplicationError::RuleNotApplicable,
    ApplicationResult, Reduction,
};
use itertools::izip;

use crate::ast::SymbolTable;

register_rule_set!("Constant", ());

#[register_rule(("Constant", 9001))]
fn apply_eval_constant(expr: &Expr, _: &SymbolTable) -> ApplicationResult {
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
        Expr::Abs(_, e) => un_op::<i32, i32>(|a| a.abs(), e).map(Lit::Int),
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
        // this is done elsewhere instead - root should return a new root with a literal inside it,
        // not a literal
        Expr::Root(_, _) => None,
        Expr::Or(_, exprs) => vec_op::<bool, bool>(|e| e.iter().any(|&e| e), exprs).map(Lit::Bool),
        Expr::Imply(_, box1, box2) => {
            let a: &Atom = (&**box1).try_into().ok()?;
            let b: &Atom = (&**box2).try_into().ok()?;

            let a: bool = a.try_into().ok()?;
            let b: bool = b.try_into().ok()?;

            if a {
                // true -> b ~> b
                Some(Lit::Bool(b))
            } else {
                // false -> b ~> true
                Some(Lit::Bool(true))
            }
        }

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

        Expr::MinionReifyImply(_, a, b) => {
            let result = eval_constant(a)?;

            let result: bool = result.try_into().ok()?;
            let b: bool = b.try_into().ok()?;

            if b {
                Some(Lit::Bool(result))
            } else {
                Some(Lit::Bool(true))
            }
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

        Expr::MinionPow(_, a, b, c) => {
            // only available for positive a b c

            let a: i32 = a.try_into().ok()?;
            let b: i32 = b.try_into().ok()?;
            let c: i32 = c.try_into().ok()?;

            if a <= 0 {
                return None;
            }

            if b <= 0 {
                return None;
            }

            if c <= 0 {
                return None;
            }

            Some(Lit::Bool(a ^ b == c))
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
        }
        Expr::FlatProductEq(_, a, b, c) => {
            let a: i32 = a.try_into().ok()?;
            let b: i32 = b.try_into().ok()?;
            let c: i32 = c.try_into().ok()?;
            Some(Lit::Bool(a * b == c))
        }
        Expr::FlatWeightedSumLeq(_, cs, vs, total) => {
            let cs: Vec<i32> = cs
                .iter()
                .map(|x| TryInto::<i32>::try_into(x).ok())
                .collect::<Option<Vec<i32>>>()?;
            let vs: Vec<i32> = vs
                .iter()
                .map(|x| TryInto::<i32>::try_into(x).ok())
                .collect::<Option<Vec<i32>>>()?;
            let total: i32 = total.try_into().ok()?;

            let sum: i32 = izip!(cs, vs).fold(0, |acc, (c, v)| acc + (c * v));

            Some(Lit::Bool(sum <= total))
        }

        Expr::FlatWeightedSumGeq(_, cs, vs, total) => {
            let cs: Vec<i32> = cs
                .iter()
                .map(|x| TryInto::<i32>::try_into(x).ok())
                .collect::<Option<Vec<i32>>>()?;
            let vs: Vec<i32> = vs
                .iter()
                .map(|x| TryInto::<i32>::try_into(x).ok())
                .collect::<Option<Vec<i32>>>()?;
            let total: i32 = total.try_into().ok()?;

            let sum: i32 = izip!(cs, vs).fold(0, |acc, (c, v)| acc + (c * v));

            Some(Lit::Bool(sum >= total))
        }
        Expr::FlatAbsEq(_, x, y) => {
            let x: i32 = x.try_into().ok()?;
            let y: i32 = y.try_into().ok()?;

            Some(Lit::Bool(x == y.abs()))
        }

        Expr::UnsafePow(_, a, b) | Expr::SafePow(_, a, b) => {
            let a: &Atom = a.try_into().ok()?;
            let a: i32 = a.try_into().ok()?;

            let b: &Atom = b.try_into().ok()?;
            let b: i32 = b.try_into().ok()?;

            if (a != 0 || b != 0) && b >= 0 {
                Some(Lit::Int(a ^ b))
            } else {
                None
            }
        }
    }
}

/// Evaluate the root expression.
///
/// This returns either Expr::Root([true]) or Expr::Root([false]).
#[register_rule(("Constant", 9001))]
fn eval_root(expr: &Expr, _: &SymbolTable) -> ApplicationResult {
    // this is its own rule not part of apply_eval_constant, because root should return a new root
    // with a literal inside it, not just a literal

    let Expr::Root(_, exprs) = expr else {
        return Err(RuleNotApplicable);
    };

    match exprs.len() {
        0 => Ok(Reduction::pure(Expr::Root(
            Metadata::new(),
            vec![true.into()],
        ))),
        1 => Err(RuleNotApplicable),
        _ => {
            let lit =
                vec_op::<bool, bool>(|e| e.iter().all(|&e| e), exprs).ok_or(RuleNotApplicable)?;

            Ok(Reduction::pure(Expr::Root(
                Metadata::new(),
                vec![lit.into()],
            )))
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

#[allow(dead_code)]
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
