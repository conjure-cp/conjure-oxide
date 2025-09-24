#![allow(dead_code)]
use conjure_cp::ast::Metadata;
use conjure_cp::ast::{
    AbstractLiteral, Atom, Expression as Expr, Literal as Lit, SymbolTable, matrix,
};
use conjure_cp::into_matrix;
use conjure_cp::rule_engine::{
    ApplicationError::RuleNotApplicable, ApplicationResult, Reduction, register_rule,
    register_rule_set,
};
use itertools::{Itertools as _, izip};
use std::collections::HashSet;
use std::sync::Arc;
use std::sync::atomic::{AtomicBool, Ordering};
use uniplate::Biplate;

use super::partial_eval::run_partial_evaluator;

register_rule_set!("Constant", ());

#[register_rule(("Constant", 9001))]
fn constant_evaluator(expr: &Expr, symtab: &SymbolTable) -> ApplicationResult {
    // I break the rules a bit here: this is a global rule!
    //
    // This rule is really really hot when expanding comprehensions.. Also, at time of writing, we
    // have the naive rewriter, which is slow on large trees....
    //
    // Also, constant_evaluating bottom up vs top down does things in less passes: the rewriter,
    // however, favour doing this top-down!
    //
    // e.g. or([(1=1),(2=2),(3+3 = 6)])

    if !matches!(expr, Expr::Root(_, _)) {
        return Err(RuleNotApplicable);
    };

    let has_changed: Arc<AtomicBool> = Arc::new(AtomicBool::new(false));
    let has_changed_2 = Arc::clone(&has_changed);

    let symtab = symtab.clone();
    let new_expr = expr.transform_bi(&move |x| {
        if let Expr::Atomic(_, Atom::Literal(_)) = x {
            return x;
        }

        match eval_constant(&x)
            .map(|c| Expr::Atomic(Metadata::new(), Atom::Literal(c)))
            .or_else(|| {
                run_partial_evaluator(&x, &symtab)
                    .ok()
                    .map(|r| r.new_expression)
            }) {
            Some(new_expr) => {
                has_changed.store(true, Ordering::Relaxed);
                new_expr
            }

            None => x,
        }
    });

    if has_changed_2.load(Ordering::Relaxed) {
        Ok(Reduction::pure(new_expr))
    } else {
        Err(RuleNotApplicable)
    }
}

/// Simplify an expression to a constant if possible
/// Returns:
/// `None` if the expression cannot be simplified to a constant (e.g. if it contains a variable)
/// `Some(Const)` if the expression can be simplified to a constant
pub fn eval_constant(expr: &Expr) -> Option<Lit> {
    match expr {
        Expr::Supset(_, a, b) => match (a.as_ref(), b.as_ref()) {
            (
                Expr::Atomic(_, Atom::Literal(Lit::AbstractLiteral(AbstractLiteral::Set(a)))),
                Expr::Atomic(_, Atom::Literal(Lit::AbstractLiteral(AbstractLiteral::Set(b)))),
            ) => {
                let a_set: HashSet<Lit> = a.iter().cloned().collect();
                let b_set: HashSet<Lit> = b.iter().cloned().collect();

                if a_set.difference(&b_set).count() > 0 {
                    Some(Lit::Bool(a_set.is_superset(&b_set)))
                } else {
                    Some(Lit::Bool(false))
                }
            }
            _ => None,
        },
        Expr::SupsetEq(_, a, b) => match (a.as_ref(), b.as_ref()) {
            (
                Expr::Atomic(_, Atom::Literal(Lit::AbstractLiteral(AbstractLiteral::Set(a)))),
                Expr::Atomic(_, Atom::Literal(Lit::AbstractLiteral(AbstractLiteral::Set(b)))),
            ) => Some(Lit::Bool(
                a.iter()
                    .cloned()
                    .collect::<HashSet<Lit>>()
                    .is_superset(&b.iter().cloned().collect::<HashSet<Lit>>()),
            )),
            _ => None,
        },
        Expr::Subset(_, a, b) => match (a.as_ref(), b.as_ref()) {
            (
                Expr::Atomic(_, Atom::Literal(Lit::AbstractLiteral(AbstractLiteral::Set(a)))),
                Expr::Atomic(_, Atom::Literal(Lit::AbstractLiteral(AbstractLiteral::Set(b)))),
            ) => {
                let a_set: HashSet<Lit> = a.iter().cloned().collect();
                let b_set: HashSet<Lit> = b.iter().cloned().collect();

                if b_set.difference(&a_set).count() > 0 {
                    Some(Lit::Bool(a_set.is_subset(&b_set)))
                } else {
                    Some(Lit::Bool(false))
                }
            }
            _ => None,
        },
        Expr::SubsetEq(_, a, b) => match (a.as_ref(), b.as_ref()) {
            (
                Expr::Atomic(_, Atom::Literal(Lit::AbstractLiteral(AbstractLiteral::Set(a)))),
                Expr::Atomic(_, Atom::Literal(Lit::AbstractLiteral(AbstractLiteral::Set(b)))),
            ) => Some(Lit::Bool(
                a.iter()
                    .cloned()
                    .collect::<HashSet<Lit>>()
                    .is_subset(&b.iter().cloned().collect::<HashSet<Lit>>()),
            )),
            _ => None,
        },
        Expr::Intersect(_, a, b) => match (a.as_ref(), b.as_ref()) {
            (
                Expr::Atomic(_, Atom::Literal(Lit::AbstractLiteral(AbstractLiteral::Set(a)))),
                Expr::Atomic(_, Atom::Literal(Lit::AbstractLiteral(AbstractLiteral::Set(b)))),
            ) => {
                let mut res: Vec<Lit> = Vec::new();
                for lit in a.iter() {
                    if b.contains(lit) && !res.contains(lit) {
                        res.push(lit.clone());
                    }
                }
                Some(Lit::AbstractLiteral(AbstractLiteral::Set(res)))
            }
            _ => None,
        },
        Expr::Union(_, a, b) => match (a.as_ref(), b.as_ref()) {
            (
                Expr::Atomic(_, Atom::Literal(Lit::AbstractLiteral(AbstractLiteral::Set(a)))),
                Expr::Atomic(_, Atom::Literal(Lit::AbstractLiteral(AbstractLiteral::Set(b)))),
            ) => {
                let mut res: Vec<Lit> = Vec::new();
                for lit in a.iter() {
                    res.push(lit.clone());
                }
                for lit in b.iter() {
                    if !res.contains(lit) {
                        res.push(lit.clone());
                    }
                }
                Some(Lit::AbstractLiteral(AbstractLiteral::Set(res)))
            }
            _ => None,
        },
        Expr::In(_, a, b) => {
            if let (
                Expr::Atomic(_, Atom::Literal(Lit::Int(c))),
                Expr::Atomic(_, Atom::Literal(Lit::AbstractLiteral(AbstractLiteral::Set(d)))),
            ) = (a.as_ref(), b.as_ref())
            {
                for lit in d.iter() {
                    if let Lit::Int(x) = lit {
                        if c == x {
                            return Some(Lit::Bool(true));
                        }
                    }
                }
                Some(Lit::Bool(false))
            } else {
                None
            }
        }
        Expr::FromSolution(_, _) => None,
        Expr::DominanceRelation(_, _) => None,
        Expr::InDomain(_, e, domain) => {
            let Expr::Atomic(_, Atom::Literal(lit)) = e.as_ref() else {
                return None;
            };

            domain.contains(lit).ok().map(Into::into)
        }
        Expr::Atomic(_, Atom::Literal(c)) => Some(c.clone()),
        Expr::Atomic(_, Atom::Reference(_)) => None,
        Expr::AbstractLiteral(_, a) => {
            if let AbstractLiteral::Set(s) = a {
                let mut copy = Vec::new();
                for expr in s.iter() {
                    if let Expr::Atomic(_, Atom::Literal(lit)) = expr {
                        copy.push(lit.clone());
                    } else {
                        return None;
                    }
                }
                Some(Lit::AbstractLiteral(AbstractLiteral::Set(copy)))
            } else {
                None
            }
        }
        Expr::Comprehension(_, _) => None,
        Expr::UnsafeIndex(_, subject, indices) | Expr::SafeIndex(_, subject, indices) => {
            let subject: Lit = subject.as_ref().clone().into_literal()?;
            let indices: Vec<Lit> = indices
                .iter()
                .cloned()
                .map(|x| x.into_literal())
                .collect::<Option<Vec<Lit>>>()?;

            match subject {
                Lit::AbstractLiteral(subject @ AbstractLiteral::Matrix(_, _)) => {
                    matrix::flatten_enumerate(subject)
                        .find(|(i, _)| i == &indices)
                        .map(|(_, x)| x)
                }
                Lit::AbstractLiteral(subject @ AbstractLiteral::Tuple(_)) => {
                    let AbstractLiteral::Tuple(elems) = subject else {
                        return None;
                    };

                    assert!(indices.len() == 1, "nested tuples not supported yet");

                    let Lit::Int(index) = indices[0].clone() else {
                        return None;
                    };

                    if elems.len() < index as usize || index < 1 {
                        return None;
                    }

                    // -1 for 0-indexing vs 1-indexing
                    let item = elems[index as usize - 1].clone();

                    Some(item)
                }
                Lit::AbstractLiteral(subject @ AbstractLiteral::Record(_)) => {
                    let AbstractLiteral::Record(elems) = subject else {
                        return None;
                    };

                    assert!(indices.len() == 1, "nested record not supported yet");

                    let Lit::Int(index) = indices[0].clone() else {
                        return None;
                    };

                    if elems.len() < index as usize || index < 1 {
                        return None;
                    }

                    // -1 for 0-indexing vs 1-indexing
                    let item = elems[index as usize - 1].clone();
                    Some(item.value)
                }
                _ => None,
            }
        }
        Expr::UnsafeSlice(_, subject, indices) | Expr::SafeSlice(_, subject, indices) => {
            let subject: Lit = subject.as_ref().clone().into_literal()?;
            let Lit::AbstractLiteral(subject @ AbstractLiteral::Matrix(_, _)) = subject else {
                return None;
            };

            let hole_dim = indices
                .iter()
                .cloned()
                .position(|x| x.is_none())
                .expect("slice expression should have a hole dimension");

            let missing_domain = matrix::index_domains(subject.clone())[hole_dim].clone();

            let indices: Vec<Option<Lit>> = indices
                .iter()
                .cloned()
                .map(|x| {
                    // the outer option represents success of this iterator, the inner the index
                    // slice.
                    match x {
                        Some(x) => x.into_literal().map(Some),
                        None => Some(None),
                    }
                })
                .collect::<Option<Vec<Option<Lit>>>>()?;

            let indices_in_slice: Vec<Vec<Lit>> = missing_domain
                .values()
                .ok()?
                .into_iter()
                .map(|i| {
                    let mut indices = indices.clone();
                    indices[hole_dim] = Some(i);
                    // These unwraps will only fail if we have multiple holes.
                    // As this is invalid, panicking is fine.
                    indices.into_iter().map(|x| x.unwrap()).collect_vec()
                })
                .collect_vec();

            // Note: indices_in_slice is not necessarily sorted, so this is the best way.
            let elems = matrix::flatten_enumerate(subject)
                .filter(|(i, _)| indices_in_slice.contains(i))
                .map(|(_, elem)| elem)
                .collect();

            Some(Lit::AbstractLiteral(into_matrix![elems]))
        }
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
        Expr::And(_, e) => {
            vec_lit_op::<bool, bool>(|e| e.iter().all(|&e| e), e.as_ref()).map(Lit::Bool)
        }
        Expr::Root(_, _) => None,
        Expr::Or(_, es) => {
            // possibly cheating; definitely should be in partial eval instead
            for e in (**es).clone().unwrap_list()? {
                if let Expr::Atomic(_, Atom::Literal(Lit::Bool(true))) = e {
                    return Some(Lit::Bool(true));
                };
            }

            vec_lit_op::<bool, bool>(|e| e.iter().any(|&e| e), es.as_ref()).map(Lit::Bool)
        }
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
        Expr::Iff(_, box1, box2) => {
            let a: &Atom = (&**box1).try_into().ok()?;
            let b: &Atom = (&**box2).try_into().ok()?;

            let a: bool = a.try_into().ok()?;
            let b: bool = b.try_into().ok()?;

            Some(Lit::Bool(a == b))
        }
        Expr::Sum(_, exprs) => vec_lit_op::<i32, i32>(|e| e.iter().sum(), exprs).map(Lit::Int),
        Expr::Product(_, exprs) => {
            vec_lit_op::<i32, i32>(|e| e.iter().product(), exprs).map(Lit::Int)
        }
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
        Expr::Min(_, e) => {
            opt_vec_lit_op::<i32, i32>(|e| e.iter().min().copied(), e.as_ref()).map(Lit::Int)
        }
        Expr::Max(_, e) => {
            opt_vec_lit_op::<i32, i32>(|e| e.iter().max().copied(), e.as_ref()).map(Lit::Int)
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
        Expr::MinionWInSet(_, _, _) => None,
        Expr::MinionWInIntervalSet(_, x, intervals) => {
            let x_lit: &Lit = x.try_into().ok()?;

            let x_lit = match x_lit.clone() {
                Lit::Int(i) => Some(i),
                Lit::Bool(true) => Some(1),
                Lit::Bool(false) => Some(0),
                _ => None,
            }?;

            let mut intervals = intervals.iter();
            loop {
                let Some(lower) = intervals.next() else {
                    break;
                };

                let Some(upper) = intervals.next() else {
                    break;
                };
                if &x_lit >= lower && &x_lit <= upper {
                    return Some(Lit::Bool(true));
                }
            }

            Some(Lit::Bool(false))
        }
        Expr::AllDiff(_, e) => {
            let es = (**e).clone().unwrap_list()?;
            let mut lits: HashSet<Lit> = HashSet::new();
            for expr in es {
                let Expr::Atomic(_, Atom::Literal(x)) = expr else {
                    return None;
                };
                match x {
                    Lit::Int(_) | Lit::Bool(_) => {
                        if lits.contains(&x) {
                            return Some(Lit::Bool(false));
                        } else {
                            lits.insert(x.clone());
                        }
                    }
                    Lit::AbstractLiteral(_) => return None, // Reject AbstractLiteral cases
                }
            }
            Some(Lit::Bool(true))
        }
        Expr::FlatAllDiff(_, es) => {
            let mut lits: HashSet<Lit> = HashSet::new();
            for atom in es {
                let Atom::Literal(x) = atom else {
                    return None;
                };

                match x {
                    Lit::Int(_) | Lit::Bool(_) => {
                        if lits.contains(x) {
                            return Some(Lit::Bool(false));
                        } else {
                            lits.insert(x.clone());
                        }
                    }
                    Lit::AbstractLiteral(_) => return None, // Reject AbstractLiteral cases
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
                Some(Lit::Int(a.pow(b as u32)))
            } else {
                None
            }
        }
        Expr::Scope(_, _) => None,
        Expr::Metavar(_, _) => None,
        Expr::MinionElementOne(_, _, _, _) => None,
        Expr::ToInt(_, expression) => {
            let lit = (**expression).clone().into_literal()?;
            match lit {
                Lit::Int(_) => Some(lit),
                Lit::Bool(true) => Some(Lit::Int(1)),
                Lit::Bool(false) => Some(Lit::Int(0)),
                _ => None,
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

fn vec_lit_op<T, A>(f: fn(Vec<T>) -> A, a: &Expr) -> Option<A>
where
    T: TryFrom<Lit>,
{
    // we don't care about preserving indices here, as we will be getting rid of the vector
    // anyways!
    let a = a.clone().unwrap_matrix_unchecked()?.0;
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

fn opt_vec_lit_op<T, A>(f: fn(Vec<T>) -> Option<A>, a: &Expr) -> Option<A>
where
    T: TryFrom<Lit>,
{
    let a = a.clone().unwrap_list()?;
    // FIXME: deal with explicit matrix domains
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
    use crate::eval_constant;
    use conjure_cp::ast::{Expression, Moo};
    use conjure_cp::essence_expr;

    #[test]
    fn div_by_zero() {
        let expr = essence_expr!(1 / 0);
        assert_eq!(eval_constant(&expr), None);
    }

    #[test]
    fn safediv_by_zero() {
        let expr = Expression::SafeDiv(
            Default::default(),
            Moo::new(essence_expr!(1)),
            Moo::new(essence_expr!(0)),
        );
        assert_eq!(eval_constant(&expr), None);
    }
}
