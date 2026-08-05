#![allow(dead_code)]
use crate::ast::{
    AbstractLiteral, Atom, DeclarationKind, Expression as Expr, Field, Literal as Lit, Metadata,
    Moo,
    comprehension::{Comprehension, ComprehensionQualifier},
    matrix,
};
use crate::into_matrix;
use itertools::{Itertools as _, izip};
use std::cmp::Ordering as CmpOrdering;
use std::collections::HashSet;
use uniplate::Uniplate;

use super::partial_eval::{run_partial_evaluator, run_partial_evaluator_local};

pub(crate) fn factorial_i32(n: i32) -> Option<i32> {
    if n < 0 {
        return None;
    }

    (1..=n).try_fold(1_i32, i32::checked_mul)
}

fn eval_constant_set(expr: &Expr) -> Option<Vec<Lit>> {
    let Lit::AbstractLiteral(AbstractLiteral::Set(values)) = eval_constant(expr)? else {
        return None;
    };

    Some(values)
}

/// Compare literals only when their outer Essence literal kinds agree.
///
/// Inner types are guaranteed by expression type checking; keeping this guard here also makes
/// direct evaluator calls on unlike primitive or abstract literal kinds safely unevaluable.
fn equal_constant_literals(lhs: &Lit, rhs: &Lit) -> Option<bool> {
    match (lhs, rhs) {
        (Lit::Int(_), Lit::Int(_)) | (Lit::Bool(_), Lit::Bool(_)) => {
            Some(lhs.essence_cmp(rhs) == CmpOrdering::Equal)
        }
        (Lit::AbstractLiteral(lhs_abstract), Lit::AbstractLiteral(rhs_abstract))
            if std::mem::discriminant(lhs_abstract) == std::mem::discriminant(rhs_abstract) =>
        {
            Some(lhs.essence_cmp(rhs) == CmpOrdering::Equal)
        }
        _ => None,
    }
}

/// Simplify an expression to a constant using only constants already present at this node.
///
/// This is intended for the rewriter: child expressions should have been simplified by the
/// scheduler before their parent is considered. Use [`eval_constant`] when a caller explicitly
/// wants recursive evaluation of an arbitrary expression.
pub fn eval_constant_local(expr: &Expr) -> Option<Lit> {
    if !has_only_local_constant_operands(expr) {
        return None;
    }

    eval_constant(expr)
}

/// Applies the evaluator normalisation hook to a focused expression.
///
/// Evaluators are privileged simplifications, not ordinary rewrite rules. The rewriter invokes
/// this hook before normal rule scheduling and immediately after a successful ordinary rule,
/// walking upward while evaluation keeps simplifying parents. This exploits the semantic property
/// that local constant and partial evaluation is always preferable to trying lower-priority rules,
/// while avoiding millions of failed universal `constant_evaluator` rule attempts.
///
/// Away from [`Expr::Root`], the hook is pure and local: it does not create auxiliaries, mutate
/// the symbol table, or recursively inspect arbitrary descendants. Children are expected to have
/// been normalised by the scheduler before their parent is evaluated.
///
/// At [`Expr::Root`], a selective deep pass runs over top-level constraints (skipping solver-flat
/// forms). Callers that know which root child changed should prefer
/// [`normalise_root_selective_deep_expr`] with `only_constraint` so sibling constraints are not
/// re-traversed. Local root-list reshaping (flatten top-level `and`) is intentionally deferred to
/// [`finish_root_evaluator_normalisation`]: doing it mid-loop materialises huge root lists and
/// explodes worklist rule attempts.
pub fn normalise_evaluator_local(expr: &Expr) -> Option<Expr> {
    match expr {
        Expr::Root(_, exprs) => normalise_root_constraints_selective_deep(exprs, None),
        // Focused `AbstractLiteral` literals must not repeatedly refold to themselves; parents
        // still see the `Atomic(Literal(...))` form when the hook walks upward.
        Expr::Atomic(_, Atom::Literal(Lit::AbstractLiteral(_))) => None,
        _ => fold_constant_expression_local(expr).or_else(|| {
            run_partial_evaluator_local(expr)
                .ok()
                .map(|reduction| reduction.new_expression)
        }),
    }
    .filter(|new_expr| new_expr != expr)
}

/// Applies local root-list partial evaluation (strip `true`, propagate `false`, flatten `and`s).
pub fn normalise_root_constraints_local(exprs: &[Expr]) -> Option<Expr> {
    if exprs.is_empty() {
        return Some(Expr::Root(Metadata::new(), vec![true.into()]));
    }

    let root = Expr::Root(Metadata::new(), exprs.to_vec());
    run_partial_evaluator_local(&root)
        .ok()
        .map(|reduction| reduction.new_expression)
        .filter(|new_root| new_root != &root)
}

/// Deep-normalises selected non-flat top-level constraints in `root`.
pub fn normalise_root_selective_deep_expr(
    root: &Expr,
    only_constraint: Option<usize>,
) -> Option<Expr> {
    let Expr::Root(_, exprs) = root else {
        return None;
    };

    normalise_root_constraints_selective_deep(exprs, only_constraint)
}

pub fn normalise_root_constraints_deep(root: &Expr) -> Option<Expr> {
    normalise_root_selective_deep_expr(root, None)
}

/// Finishes evaluator normalisation on the model root after rewriting completes.
///
/// Applies local root-list partial evaluation to a fixpoint (strip `true`, propagate `false`,
/// flatten top-level `and`). Deep evaluation of individual constraints already runs during
/// rewriting via [`normalise_evaluator_local`] / [`normalise_root_selective_deep_expr`]; this
/// finish pass must not be inlined into the mid-loop Root hook because flattening `and` early
/// explodes worklist size on large expansions.
pub fn finish_root_evaluator_normalisation(root: &Expr) -> Option<Expr> {
    let Expr::Root(_, exprs) = root else {
        return None;
    };

    let mut current = Expr::Root(Metadata::new(), exprs.clone());
    let mut changed = false;
    while let Expr::Root(_, current_exprs) = &current {
        let Some(next) = normalise_root_constraints_local(current_exprs) else {
            break;
        };
        current = next;
        changed = true;
    }
    changed.then_some(current)
}

fn normalise_root_constraints_selective_deep(
    exprs: &[Expr],
    only_constraint: Option<usize>,
) -> Option<Expr> {
    if exprs.is_empty() {
        return Some(Expr::Root(Metadata::new(), vec![true.into()]));
    }

    let mut changed = false;
    let constraints = exprs
        .iter()
        .enumerate()
        .map(|(index, constraint)| {
            if only_constraint.is_some_and(|only| only != index) {
                return constraint.clone();
            }

            if constraint_skips_deep_root_normalisation(constraint) {
                return constraint.clone();
            }

            if let Some(normalised) = normalise_constraint_deep_to_fixpoint(constraint) {
                changed = true;
                normalised
            } else {
                constraint.clone()
            }
        })
        .collect();

    changed.then(|| Expr::Root(Metadata::new(), constraints))
}

/// Whether a top-level constraint has already been lowered to solver-flat form.
///
/// Deep root normalisation must not rewrite these: doing so can disturb auxiliaries introduced
/// for Minion (for example chained `FlatProductEq` constraints) or repeat expensive work.
fn constraint_skips_deep_root_normalisation(expr: &Expr) -> bool {
    match expr {
        Expr::FlatProductEq(_, _, _, _)
        | Expr::FlatSumLeq(_, _, _)
        | Expr::FlatSumGeq(_, _, _)
        | Expr::FlatMinEq(_, _, _)
        | Expr::FlatIneq(_, _, _, _)
        | Expr::FlatMinusEq(_, _, _)
        | Expr::FlatAbsEq(_, _, _)
        | Expr::FlatAllDiff(_, _)
        | Expr::FlatWeightedSumLeq(_, _, _, _)
        | Expr::FlatWeightedSumGeq(_, _, _, _)
        | Expr::FlatWatchedLiteral(_, _, _)
        | Expr::MinionDivEqUndefZero(_, _, _, _)
        | Expr::MinionModuloEqUndefZero(_, _, _, _)
        | Expr::MinionPow(_, _, _, _)
        | Expr::MinionReify(_, _, _)
        | Expr::MinionReifyImply(_, _, _)
        | Expr::MinionWInIntervalSet(_, _, _)
        | Expr::MinionWInSet(_, _, _)
        | Expr::MinionElementOne(_, _, _, _) => true,
        Expr::AuxDeclaration(_, _, inner) => matches!(
            inner.as_ref(),
            Expr::Product(_, _) | Expr::FlatProductEq(_, _, _, _)
        ),
        _ => false,
    }
}

fn fold_constant_expression_deep(expr: &Expr) -> Option<Expr> {
    let constant = eval_constant(expr)?;
    fold_constant_expression(expr, constant).filter(|folded| folded != expr)
}

fn normalise_constraint_deep_to_fixpoint(constraint: &Expr) -> Option<Expr> {
    if matches!(constraint, Expr::Atomic(_, Atom::Literal(_))) {
        return None;
    }

    let mut current = constraint.clone();
    let mut changed = false;

    while let Some(step) = fold_constant_expression_deep(&current)
        .or_else(|| partial_evaluator_deep_step(&current))
        .filter(|step| step != &current)
    {
        current = step;
        changed = true;
    }

    changed
        .then_some(current)
        .filter(|current| current != constraint)
}

fn partial_evaluator_deep_step(expr: &Expr) -> Option<Expr> {
    run_partial_evaluator(expr)
        .ok()
        .map(|reduction| reduction.new_expression)
        .filter(|new_expr| new_expr != expr)
}

/// Constant-folds `expr` locally unless doing so would inline a referenced matrix literal.
fn fold_constant_expression_local(expr: &Expr) -> Option<Expr> {
    let constant = match expr {
        // Comprehensions are atomic in arena traversal; evaluate them in one step here rather than
        // via operand checks that would repeat the same work for every parent.
        Expr::Comprehension(_, _) => eval_constant(expr)?,
        _ => eval_constant_local(expr)?,
    };
    fold_constant_expression(expr, constant).filter(|folded| folded != expr)
}

fn fold_constant_expression(expr: &Expr, constant: Lit) -> Option<Expr> {
    if let Expr::Atomic(_, Atom::Literal(existing)) = expr
        && existing == &constant
    {
        return None;
    }

    if matches!(
        (expr, constant.clone()),
        (
            Expr::Atomic(_, Atom::Reference(_)),
            Lit::AbstractLiteral(AbstractLiteral::Matrix(_, _))
        )
    ) {
        return None;
    }

    let folded = Expr::Atomic(Metadata::new(), Atom::Literal(constant));
    if let Expr::TypeAnnotation(_, _, domain) = expr
        && let Expr::Atomic(
            _,
            Atom::Literal(Lit::AbstractLiteral(AbstractLiteral::Matrix(elems, _))),
        ) = &folded
        && elems.is_empty()
    {
        return Some(Expr::TypeAnnotation(
            Metadata::new(),
            Moo::new(folded),
            domain.clone(),
        ));
    }

    if let Expr::DomainAnnotation(_, _, domain) = expr
        && let Expr::Atomic(
            _,
            Atom::Literal(Lit::AbstractLiteral(AbstractLiteral::Matrix(elems, _))),
        ) = &folded
        && elems.is_empty()
    {
        return Some(Expr::DomainAnnotation(
            Metadata::new(),
            Moo::new(folded),
            domain.clone(),
        ));
    }

    if let Expr::Comprehension(_, comprehension) = expr
        && let Expr::Atomic(
            _,
            Atom::Literal(Lit::AbstractLiteral(AbstractLiteral::Matrix(elems, _))),
        ) = &folded
        && elems.is_empty()
        && let Some(domain) = comprehension.domain_of()
    {
        return Some(Expr::DomainAnnotation(
            Metadata::new(),
            Moo::new(folded),
            domain,
        ));
    }

    Some(folded)
}

fn has_only_local_constant_operands(expr: &Expr) -> bool {
    match expr {
        Expr::Atomic(_, Atom::Literal(_)) => true,
        Expr::Atomic(_, Atom::Reference(reference)) => reference.resolve_constant().is_some(),
        Expr::AbstractLiteral(_, lit) => abstract_literal_children_are_local_constants(lit),
        Expr::TypeAnnotation(_, inner, _) | Expr::DomainAnnotation(_, inner, _) => {
            is_local_constant_expr(inner.as_ref())
        }
        Expr::Comprehension(_, _) | Expr::AbstractComprehension(_, _) | Expr::Root(_, _) => false,
        _ => expr.children().iter().all(is_local_constant_expr),
    }
}

fn is_local_constant_expr(expr: &Expr) -> bool {
    match expr {
        Expr::Atomic(_, Atom::Literal(_)) => true,
        Expr::Atomic(_, Atom::Reference(reference)) => reference.resolve_constant().is_some(),
        Expr::AbstractLiteral(_, lit) => abstract_literal_children_are_local_constants(lit),
        Expr::TypeAnnotation(_, inner, _) | Expr::DomainAnnotation(_, inner, _) => {
            is_local_constant_expr(inner.as_ref())
        }
        _ => false,
    }
}

fn abstract_literal_children_are_local_constants(lit: &AbstractLiteral<Expr>) -> bool {
    match lit {
        AbstractLiteral::Set(items)
        | AbstractLiteral::MSet(items)
        | AbstractLiteral::Tuple(items)
        | AbstractLiteral::Matrix(items, _) => items.iter().all(is_local_constant_expr),
        AbstractLiteral::Record(fields) => fields
            .iter()
            .all(|field| is_local_constant_expr(&field.value)),
        AbstractLiteral::Sequence(items) => items.iter().all(is_local_constant_expr),
        AbstractLiteral::Function(items) => items
            .iter()
            .all(|(from, to)| is_local_constant_expr(from) && is_local_constant_expr(to)),
        AbstractLiteral::Relation(items) => items
            .iter()
            .all(|tuple| tuple.iter().all(is_local_constant_expr)),
        AbstractLiteral::Partition(parts) => parts
            .iter()
            .all(|part| part.iter().all(is_local_constant_expr)),
        AbstractLiteral::Variant(field) => is_local_constant_expr(&field.value),
    }
}

/// Simplify an expression to a constant if possible
/// Returns:
/// `None` if the expression cannot be simplified to a constant (e.g. if it contains a variable)
/// `Some(Const)` if the expression can be simplified to a constant
pub fn eval_constant(expr: &Expr) -> Option<Lit> {
    match expr {
        Expr::TypeAnnotation(_, expr, _) | Expr::DomainAnnotation(_, expr, _) => {
            eval_constant(expr)
        }
        Expr::Supset(_, a, b) => {
            let (
                Lit::AbstractLiteral(AbstractLiteral::Set(a)),
                Lit::AbstractLiteral(AbstractLiteral::Set(b)),
            ) = (eval_constant(a.as_ref())?, eval_constant(b.as_ref())?)
            else {
                return None;
            };

            let a_set: HashSet<Lit> = a.iter().cloned().collect();
            let b_set: HashSet<Lit> = b.iter().cloned().collect();

            if a_set.difference(&b_set).count() > 0 {
                Some(Lit::Bool(a_set.is_superset(&b_set)))
            } else {
                Some(Lit::Bool(false))
            }
        }
        Expr::SupsetEq(_, a, b) => {
            let (
                Lit::AbstractLiteral(AbstractLiteral::Set(a)),
                Lit::AbstractLiteral(AbstractLiteral::Set(b)),
            ) = (eval_constant(a.as_ref())?, eval_constant(b.as_ref())?)
            else {
                return None;
            };

            Some(Lit::Bool(
                a.iter()
                    .cloned()
                    .collect::<HashSet<Lit>>()
                    .is_superset(&b.iter().cloned().collect::<HashSet<Lit>>()),
            ))
        }
        Expr::Subset(_, a, b) => {
            let (
                Lit::AbstractLiteral(AbstractLiteral::Set(a)),
                Lit::AbstractLiteral(AbstractLiteral::Set(b)),
            ) = (eval_constant(a.as_ref())?, eval_constant(b.as_ref())?)
            else {
                return None;
            };

            let a_set: HashSet<Lit> = a.iter().cloned().collect();
            let b_set: HashSet<Lit> = b.iter().cloned().collect();

            if b_set.difference(&a_set).count() > 0 {
                Some(Lit::Bool(a_set.is_subset(&b_set)))
            } else {
                Some(Lit::Bool(false))
            }
        }
        Expr::SubsetEq(_, a, b) => {
            let (
                Lit::AbstractLiteral(AbstractLiteral::Set(a)),
                Lit::AbstractLiteral(AbstractLiteral::Set(b)),
            ) = (eval_constant(a.as_ref())?, eval_constant(b.as_ref())?)
            else {
                return None;
            };

            Some(Lit::Bool(
                a.iter()
                    .cloned()
                    .collect::<HashSet<Lit>>()
                    .is_subset(&b.iter().cloned().collect::<HashSet<Lit>>()),
            ))
        }
        Expr::Intersect(_, a, b) => {
            let (
                Lit::AbstractLiteral(AbstractLiteral::Set(a)),
                Lit::AbstractLiteral(AbstractLiteral::Set(b)),
            ) = (eval_constant(a.as_ref())?, eval_constant(b.as_ref())?)
            else {
                return None;
            };

            let mut res: Vec<Lit> = Vec::new();
            for lit in a {
                if b.contains(&lit) && !res.contains(&lit) {
                    res.push(lit);
                }
            }
            Some(Lit::AbstractLiteral(AbstractLiteral::Set(res)))
        }
        Expr::Union(_, a, b) => {
            let (
                Lit::AbstractLiteral(AbstractLiteral::Set(a)),
                Lit::AbstractLiteral(AbstractLiteral::Set(b)),
            ) = (eval_constant(a.as_ref())?, eval_constant(b.as_ref())?)
            else {
                return None;
            };

            let mut res: Vec<Lit> = Vec::new();
            for lit in a {
                res.push(lit);
            }
            for lit in b {
                if !res.contains(&lit) {
                    res.push(lit);
                }
            }
            Some(Lit::AbstractLiteral(AbstractLiteral::Set(res)))
        }
        Expr::In(_, a, b) => {
            let member = eval_constant(a)?;
            let collection = eval_constant(b)?;
            let values = generator_values_from_constant_collection(&collection)?;
            Some(Lit::Bool(values.iter().any(|value| {
                value.essence_cmp(&member) == CmpOrdering::Equal
            })))
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
        Expr::Atomic(_, Atom::Reference(reference)) => reference.resolve_constant(),
        Expr::AbstractLiteral(_, a) => Some(Lit::AbstractLiteral(a.clone().into_literals()?)),
        Expr::Comprehension(_, comprehension) => {
            eval_constant_comprehension(comprehension.as_ref())
        }
        Expr::AbstractComprehension(_, _) => None,
        Expr::RecordField(_, rec, fld_name) => match eval_constant(rec.as_ref())? {
            Lit::AbstractLiteral(AbstractLiteral::Record(ents)) => {
                for Field { name, value } in ents {
                    if name.eq(fld_name) {
                        return Some(value);
                    }
                }
                None
            }
            Lit::AbstractLiteral(AbstractLiteral::Variant(field)) if field.name == *fld_name => {
                Some(field.value.clone())
            }
            _ => None,
        },
        Expr::UnsafeIndex(_, subject, indices) | Expr::SafeIndex(_, subject, indices) => {
            let subject: Lit = eval_constant(subject.as_ref())?;
            let indices: Vec<Lit> = indices
                .iter()
                .map(eval_constant)
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
            let subject: Lit = eval_constant(subject.as_ref())?;
            let Lit::AbstractLiteral(subject @ AbstractLiteral::Matrix(_, _)) = subject else {
                return None;
            };

            let hole_dim = indices
                .iter()
                .cloned()
                .position(|x| x.is_none())
                .expect("slice expression should have a hole dimension");

            let missing_domain = matrix::index_domains(&subject)[hole_dim].clone();

            let indices: Vec<Option<Lit>> = indices
                .iter()
                .cloned()
                .map(|x| {
                    // the outer option represents success of this iterator, the inner the index
                    // slice.
                    match x {
                        Some(x) => eval_constant(&x).map(Some),
                        None => Some(None),
                    }
                })
                .collect::<Option<Vec<Option<Lit>>>>()?;

            let indices_in_slice: Vec<Vec<Lit>> = missing_domain
                .values()
                .ok()?
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
        Expr::Eq(_, a, b) => Some(Lit::Bool(equal_constant_literals(
            &eval_constant(a)?,
            &eval_constant(b)?,
        )?)),
        Expr::Neq(_, a, b) => Some(Lit::Bool(!equal_constant_literals(
            &eval_constant(a)?,
            &eval_constant(b)?,
        )?)),
        Expr::Lt(_, a, b) => bin_op::<i32, bool>(|a, b| a < b, a, b).map(Lit::Bool),
        Expr::Gt(_, a, b) => bin_op::<i32, bool>(|a, b| a > b, a, b).map(Lit::Bool),
        Expr::Leq(_, a, b) => bin_op::<i32, bool>(|a, b| a <= b, a, b).map(Lit::Bool),
        Expr::Geq(_, a, b) => bin_op::<i32, bool>(|a, b| a >= b, a, b).map(Lit::Bool),
        Expr::Not(_, expr) => un_op::<bool, bool>(|e| !e, expr).map(Lit::Bool),
        Expr::And(_, e) => {
            vec_lit_op::<bool, bool>(|e| e.iter().all(|&e| e), e.as_ref()).map(Lit::Bool)
        }
        Expr::Table(_, _, _) => None,
        Expr::NegativeTable(_, _, _) => None,
        Expr::AtLeast(_, _, _, _) => None,
        Expr::AtMost(_, _, _, _) => None,
        Expr::Gcc(_, _, _, _) | Expr::GccWeak(_, _, _, _) => None,
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
        Expr::FlatMinEq(_, vars, result) => {
            let min = vars
                .iter()
                .try_fold(None, |acc: Option<i32>, atom: &Atom| {
                    let n: i32 = atom.try_into().ok()?;
                    Some(Some(acc.map_or(n, |m| m.min(n))))
                })??;
            let result: i32 = result.try_into().ok()?;
            Some(Lit::Bool(min == result))
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
        Expr::Substring(_, s, t) => match (s.as_ref(), t.as_ref()) {
            (
                Expr::Atomic(_, Atom::Literal(Lit::AbstractLiteral(AbstractLiteral::Sequence(s)))),
                Expr::Atomic(_, Atom::Literal(Lit::AbstractLiteral(AbstractLiteral::Sequence(t)))),
            ) => {
                if s.len() > t.len() {
                    return Some(Lit::Bool(false));
                }

                let found = t.windows(s.len()).any(|window| window == s.as_slice());
                Some(Lit::Bool(found))
            }
            _ => None,
        },
        Expr::Subsequence(_, s, t) => match (s.as_ref(), t.as_ref()) {
            (
                Expr::Atomic(_, Atom::Literal(Lit::AbstractLiteral(AbstractLiteral::Sequence(s)))),
                Expr::Atomic(_, Atom::Literal(Lit::AbstractLiteral(AbstractLiteral::Sequence(t)))),
            ) => {
                let mut i = 0;
                let mut j = 0;

                while i < s.len() && j < t.len() {
                    if s[i] == t[j] {
                        i += 1;
                    }
                    j += 1;
                }

                Some(Lit::Bool(i == s.len()))
            }
            _ => None,
        },
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
            while let Some(lower) = intervals.next() {
                let Some(upper) = intervals.next() else {
                    break;
                };
                if &x_lit >= lower && &x_lit <= upper {
                    return Some(Lit::Bool(true));
                }
            }

            Some(Lit::Bool(false))
        }
        Expr::Flatten(_, _, _) => {
            // TODO
            None
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
        Expr::Neg(_, a) => match eval_constant(a.as_ref())? {
            Lit::Int(a) => Some(Lit::Int(-a)),
            _ => None,
        },
        Expr::Factorial(_, a) => match eval_constant(a.as_ref())? {
            Lit::Int(a) => factorial_i32(a).map(Lit::Int),
            _ => None,
        },
        Expr::Minus(_, a, b) => bin_op::<i32, i32>(|a, b| a - b, a, b).map(Lit::Int),
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
        Expr::Metavar(_, _) => None,
        Expr::MinionElementOne(_, _, _, _) => None,
        Expr::ToInt(_, expression) => {
            let lit = eval_constant(expression.as_ref())?;
            match lit {
                Lit::Int(_) => Some(lit),
                Lit::Bool(true) => Some(Lit::Int(1)),
                Lit::Bool(false) => Some(Lit::Int(0)),
                _ => None,
            }
        }
        Expr::SATInt(_, _, _, _) => {
            // TODO: If this SATInt is composed of literals, we should evaluate it back to an
            // integer literal.
            //
            // This is important because `is_all_constant` currently returns true for SATInts
            // containing no references. If we don't evaluate them here, bubble rules will skip
            // them (thinking they'll be constant-folded later), but they'll actually reach
            // the solver adaptors as un-encoded unsafe operations, causing panics.
            None
        }
        Expr::PairwiseSum(_, a, b) => {
            match (eval_constant(a.as_ref())?, eval_constant(b.as_ref())?) {
                (Lit::Int(a_int), Lit::Int(b_int)) => Some(Lit::Int(a_int + b_int)),
                _ => None,
            }
        }
        Expr::PairwiseProduct(_, a, b) => {
            match (eval_constant(a.as_ref())?, eval_constant(b.as_ref())?) {
                (Lit::Int(a_int), Lit::Int(b_int)) => Some(Lit::Int(a_int * b_int)),
                _ => None,
            }
        }
        Expr::Defined(_, f) => {
            let Lit::AbstractLiteral(AbstractLiteral::Function(pairs)) = eval_constant(f)? else {
                return None;
            };
            Some(Lit::AbstractLiteral(AbstractLiteral::Set(
                pairs.into_iter().map(|(key, _)| key).collect(),
            )))
        }
        Expr::Range(_, f) => {
            let Lit::AbstractLiteral(AbstractLiteral::Function(pairs)) = eval_constant(f)? else {
                return None;
            };
            let mut values = Vec::new();
            for (_, value) in pairs {
                if !values.contains(&value) {
                    values.push(value);
                }
            }
            Some(Lit::AbstractLiteral(AbstractLiteral::Set(values)))
        }
        Expr::Image(_, f, arg) => {
            let Lit::AbstractLiteral(AbstractLiteral::Function(pairs)) = eval_constant(f)? else {
                return None;
            };
            let arg = eval_constant(arg)?;
            pairs
                .into_iter()
                .find(|(key, _)| *key == arg)
                .map(|(_, value)| value)
        }
        Expr::PreImage(_, f, img) => {
            let Lit::AbstractLiteral(AbstractLiteral::Function(pairs)) = eval_constant(f)? else {
                return None;
            };
            let img = eval_constant(img)?;
            let mut keys = Vec::new();
            for (key, value) in pairs {
                if value == img && !keys.contains(&key) {
                    keys.push(key);
                }
            }
            Some(Lit::AbstractLiteral(AbstractLiteral::Set(keys)))
        }
        // Not yet needed by any in-scope function case; the partial evaluator already refuses
        // these gracefully (Err(RuleNotApplicable)) rather than panicking.
        Expr::ImageSet(_, _, _) => None,
        Expr::Inverse(_, _, _) => None,
        Expr::Restrict(_, _, _) => None,
        Expr::ToSet(_, _) => None,
        Expr::ToMSet(_, _) => None,
        Expr::ToRelation(_, _) => None,
        Expr::Active(_, variant, alternative) => {
            let Lit::AbstractLiteral(AbstractLiteral::Variant(field)) =
                eval_constant(variant.as_ref())?
            else {
                return None;
            };
            Some(Lit::Bool(field.name == *alternative))
        }
        Expr::RelationProj(_, _, _) => todo!(),
        Expr::Apart(_, _, _) => todo!(),
        Expr::Together(_, _, _) => todo!(),
        Expr::Participants(_, _) => todo!(),
        Expr::Party(_, _, _) => todo!(),
        Expr::Parts(_, _) => todo!(),
        Expr::Card(_, collection) => {
            let Lit::AbstractLiteral(collection) = eval_constant(collection)? else {
                return None;
            };
            let length = match collection {
                AbstractLiteral::Set(values)
                | AbstractLiteral::MSet(values)
                | AbstractLiteral::Sequence(values)
                | AbstractLiteral::Matrix(values, _) => values.len(),
                AbstractLiteral::Function(entries) => entries.len(),
                AbstractLiteral::Relation(entries) => entries.len(),
                _ => return None,
            };
            i32::try_from(length).ok().map(Lit::Int)
        }
        Expr::LexLt(_, a, b) => {
            let lt = vec_expr_pairs_op::<i32, _>(a, b, |pairs, (a_len, b_len)| {
                pairs
                    .iter()
                    .find_map(|(a, b)| match a.cmp(b) {
                        CmpOrdering::Less => Some(true),     // First difference is <
                        CmpOrdering::Greater => Some(false), // First difference is >
                        CmpOrdering::Equal => None,          // No difference
                    })
                    .unwrap_or(a_len < b_len) // [1,1] <lex [1,1,x]
            })?;
            Some(lt.into())
        }
        Expr::LexLeq(_, a, b) => {
            let lt = vec_expr_pairs_op::<i32, _>(a, b, |pairs, (a_len, b_len)| {
                pairs
                    .iter()
                    .find_map(|(a, b)| match a.cmp(b) {
                        CmpOrdering::Less => Some(true),
                        CmpOrdering::Greater => Some(false),
                        CmpOrdering::Equal => None,
                    })
                    .unwrap_or(a_len <= b_len) // [1,1] <=lex [1,1,x]
            })?;
            Some(lt.into())
        }
        Expr::LexGt(_, a, b) => eval_constant(&Expr::LexLt(Metadata::new(), b.clone(), a.clone())),
        Expr::LexGeq(_, a, b) => {
            eval_constant(&Expr::LexLeq(Metadata::new(), b.clone(), a.clone()))
        }
        Expr::FlatLexLt(_, a, b) => {
            let lt = atoms_pairs_op::<i32, _>(a, b, |pairs, (a_len, b_len)| {
                pairs
                    .iter()
                    .find_map(|(a, b)| match a.cmp(b) {
                        CmpOrdering::Less => Some(true),
                        CmpOrdering::Greater => Some(false),
                        CmpOrdering::Equal => None,
                    })
                    .unwrap_or(a_len < b_len)
            })?;
            Some(lt.into())
        }
        Expr::FlatLexLeq(_, a, b) => {
            let lt = atoms_pairs_op::<i32, _>(a, b, |pairs, (a_len, b_len)| {
                pairs
                    .iter()
                    .find_map(|(a, b)| match a.cmp(b) {
                        CmpOrdering::Less => Some(true),
                        CmpOrdering::Greater => Some(false),
                        CmpOrdering::Equal => None,
                    })
                    .unwrap_or(a_len <= b_len)
            })?;
            Some(lt.into())
        }
        Expr::AllDifferentExcept(_, _, _) | Expr::ElementId(_, _, _) => None,
    }
}

pub fn un_op<T, A>(f: fn(T) -> A, a: &Expr) -> Option<A>
where
    T: TryFrom<Lit>,
{
    let a = unwrap_expr::<T>(a)?;
    Some(f(a))
}

pub fn bin_op<T, A>(f: fn(T, T) -> A, a: &Expr, b: &Expr) -> Option<A>
where
    T: TryFrom<Lit>,
{
    let a = unwrap_expr::<T>(a)?;
    let b = unwrap_expr::<T>(b)?;
    Some(f(a, b))
}

#[allow(dead_code)]
pub fn tern_op<T, A>(f: fn(T, T, T) -> A, a: &Expr, b: &Expr, c: &Expr) -> Option<A>
where
    T: TryFrom<Lit>,
{
    let a = unwrap_expr::<T>(a)?;
    let b = unwrap_expr::<T>(b)?;
    let c = unwrap_expr::<T>(c)?;
    Some(f(a, b, c))
}

pub fn vec_op<T, A>(f: fn(Vec<T>) -> A, a: &[Expr]) -> Option<A>
where
    T: TryFrom<Lit>,
{
    let a = a.iter().map(unwrap_expr).collect::<Option<Vec<T>>>()?;
    Some(f(a))
}

pub fn vec_lit_op<T, A>(f: fn(Vec<T>) -> A, a: &Expr) -> Option<A>
where
    T: TryFrom<Lit>,
{
    Some(f(eval_list_items(a)?))
}

type PairsCallback<T, A> = fn(Vec<(T, T)>, (usize, usize)) -> A;

/// Calls the given function on each consecutive pair of elements in the list expressions.
/// Also passes the length of the two lists.
fn vec_expr_pairs_op<T, A>(a: &Expr, b: &Expr, f: PairsCallback<T, A>) -> Option<A>
where
    T: TryFrom<Lit>,
{
    let a_exprs = a.clone().unwrap_matrix_unchecked()?.0;
    let b_exprs = b.clone().unwrap_matrix_unchecked()?.0;
    let lens = (a_exprs.len(), b_exprs.len());

    let lit_pairs = std::iter::zip(a_exprs, b_exprs)
        .map(|(a, b)| Some((unwrap_expr(&a)?, unwrap_expr(&b)?)))
        .collect::<Option<Vec<(T, T)>>>()?;
    Some(f(lit_pairs, lens))
}

/// Same as [`vec_expr_pairs_op`], but over slices of atoms.
fn atoms_pairs_op<T, A>(a: &[Atom], b: &[Atom], f: PairsCallback<T, A>) -> Option<A>
where
    T: TryFrom<Atom>,
{
    let lit_pairs = Iterator::zip(a.iter(), b.iter())
        .map(|(a, b)| Some((a.clone().try_into().ok()?, b.clone().try_into().ok()?)))
        .collect::<Option<Vec<(T, T)>>>()?;
    Some(f(lit_pairs, (a.len(), b.len())))
}

pub fn opt_vec_op<T, A>(f: fn(Vec<T>) -> Option<A>, a: &[Expr]) -> Option<A>
where
    T: TryFrom<Lit>,
{
    let a = a.iter().map(unwrap_expr).collect::<Option<Vec<T>>>()?;
    f(a)
}

pub fn opt_vec_lit_op<T, A>(f: fn(Vec<T>) -> Option<A>, a: &Expr) -> Option<A>
where
    T: TryFrom<Lit>,
{
    f(eval_list_items(a)?)
}

#[allow(dead_code)]
pub fn flat_op<T, A>(f: fn(Vec<T>, T) -> A, a: &[Expr], b: &Expr) -> Option<A>
where
    T: TryFrom<Lit>,
{
    let a = a.iter().map(unwrap_expr).collect::<Option<Vec<T>>>()?;
    let b = unwrap_expr::<T>(b)?;
    Some(f(a, b))
}

pub fn unwrap_expr<T: TryFrom<Lit>>(expr: &Expr) -> Option<T> {
    let c = eval_constant(expr)?;
    TryInto::<T>::try_into(c).ok()
}

fn eval_list_items<T>(expr: &Expr) -> Option<Vec<T>>
where
    T: TryFrom<Lit>,
{
    if let Some(items) = expr
        .clone()
        .unwrap_matrix_unchecked()
        .map(|(items, _)| items)
    {
        return items.iter().map(unwrap_expr).collect();
    }

    let collection = eval_constant(expr)?;
    generator_values_from_constant_collection(&collection)?
        .into_iter()
        .map(TryInto::try_into)
        .collect::<Result<Vec<_>, _>>()
        .ok()
}

fn eval_constant_comprehension(comprehension: &Comprehension) -> Option<Lit> {
    let mut values = Vec::new();
    eval_comprehension_qualifiers(comprehension, 0, &mut values)?;
    Some(Lit::AbstractLiteral(
        AbstractLiteral::matrix_implied_indices(values),
    ))
}

fn eval_comprehension_qualifiers(
    comprehension: &Comprehension,
    qualifier_index: usize,
    values: &mut Vec<Lit>,
) -> Option<()> {
    if qualifier_index == comprehension.qualifiers.len() {
        values.push(eval_constant(&comprehension.return_expression)?);
        return Some(());
    }

    match &comprehension.qualifiers[qualifier_index] {
        ComprehensionQualifier::Generator { ptr } => {
            let domain = ptr.domain()?;
            let generator_values = domain
                .resolve()
                .and_then(|x| x.values())
                .ok()?
                .collect_vec();

            for value in generator_values {
                with_temporary_quantified_binding(ptr, &value, || {
                    eval_comprehension_qualifiers(comprehension, qualifier_index + 1, values)
                })?;
            }
        }
        ComprehensionQualifier::ExpressionGenerator { ptr } => {
            // clone immediately so the read lock guard is dropped
            let expr = ptr.as_quantified_expr()?.clone();
            let generator_values = generator_values_from_expr(&expr)?;

            for value in generator_values {
                with_temporary_quantified_binding(ptr, &value, || {
                    eval_comprehension_qualifiers(comprehension, qualifier_index + 1, values)
                })?;
            }
        }
        ComprehensionQualifier::Condition(condition) => match eval_constant(condition)? {
            Lit::Bool(true) => {
                eval_comprehension_qualifiers(comprehension, qualifier_index + 1, values)?
            }
            Lit::Bool(false) => {}
            _ => return None,
        },
    }

    Some(())
}

/// Values for a constant collection expression used during constant folding.
///
/// This does not enumerate decision-variable domains; quantification over decisions is not
/// unrolled here.
pub fn generator_values_from_expr(expr: &Expr) -> Option<Vec<Lit>> {
    generator_values_from_constant_collection(&eval_constant(expr)?)
}

pub(crate) fn generator_values_from_constant_collection(lit: &Lit) -> Option<Vec<Lit>> {
    match lit {
        Lit::AbstractLiteral(AbstractLiteral::Set(values))
        | Lit::AbstractLiteral(AbstractLiteral::MSet(values))
        | Lit::AbstractLiteral(AbstractLiteral::Tuple(values)) => Some(values.clone()),
        Lit::AbstractLiteral(AbstractLiteral::Matrix(values, _)) => Some(values.clone()),
        Lit::AbstractLiteral(list) => list.unwrap_list().cloned(),
        _ => None,
    }
}

fn with_temporary_quantified_binding<T>(
    quantified: &crate::ast::DeclarationPtr,
    value: &Lit,
    f: impl FnOnce() -> Option<T>,
) -> Option<T> {
    let mut targets = vec![quantified.clone()];
    if let DeclarationKind::Quantified(inner) = &*quantified.kind()
        && let Some(generator) = inner.generator()
    {
        targets.push(generator.clone());
    }

    let mut originals = Vec::with_capacity(targets.len());
    for mut target in targets {
        let old_kind = target.replace_kind(DeclarationKind::TemporaryValueLetting(Expr::Atomic(
            Metadata::new(),
            Atom::Literal(value.clone()),
        )));
        originals.push((target, old_kind));
    }

    let result = f();

    for (mut target, old_kind) in originals.into_iter().rev() {
        let _ = target.replace_kind(old_kind);
    }

    result
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::matrix_expr;

    fn int_lit(value: i32) -> Expr {
        Expr::Atomic(Metadata::new(), Atom::Literal(Lit::Int(value)))
    }

    fn bool_lit(value: bool) -> Expr {
        Expr::Atomic(Metadata::new(), Atom::Literal(Lit::Bool(value)))
    }

    fn variant_lit(alternative: &str, value: Expr) -> Expr {
        Expr::AbstractLiteral(
            Metadata::new(),
            AbstractLiteral::Variant(Moo::new(Field {
                name: crate::ast::Name::user(alternative),
                value,
            })),
        )
    }

    fn root(exprs: Vec<Expr>) -> Expr {
        Expr::Root(Metadata::new(), exprs)
    }

    #[test]
    fn evaluates_active_on_variant_literals() {
        let variant = Moo::new(variant_lit("value", int_lit(2)));
        let active = Expr::Active(
            Metadata::new(),
            variant.clone(),
            crate::ast::Name::user("value"),
        );
        let inactive = Expr::Active(Metadata::new(), variant, crate::ast::Name::user("flag"));

        assert_eq!(eval_constant(&active), Some(Lit::Bool(true)));
        assert_eq!(eval_constant(&inactive), Some(Lit::Bool(false)));
        assert!(run_partial_evaluator_local(&active).is_ok());

        let field = Expr::RecordField(
            Metadata::new(),
            Moo::new(variant_lit("value", int_lit(2))),
            crate::ast::Name::user("value"),
        );
        assert_eq!(eval_constant(&field), Some(Lit::Int(2)));
    }

    #[test]
    fn local_root_partial_eval_strips_true_constraints() {
        let expr = root(vec![bool_lit(true), int_lit(1)]);
        let Expr::Root(_, exprs) = &expr else {
            panic!("expected root");
        };
        let normalised = normalise_root_constraints_local(exprs).unwrap();
        assert_eq!(normalised, root(vec![int_lit(1)]));
    }

    #[test]
    fn local_root_partial_eval_propagates_false() {
        let expr = root(vec![bool_lit(false), int_lit(1)]);
        let Expr::Root(_, exprs) = &expr else {
            panic!("expected root");
        };
        let normalised = normalise_root_constraints_local(exprs).unwrap();
        assert_eq!(normalised, root(vec![bool_lit(false)]));
    }

    #[test]
    fn deep_root_normalisation_folds_ground_constraint() {
        let expr = root(vec![Expr::Sum(
            Metadata::new(),
            Moo::new(matrix_expr![int_lit(1), int_lit(2), int_lit(3)]),
        )]);
        let normalised = normalise_root_constraints_deep(&expr).unwrap();
        assert_eq!(normalised, root(vec![int_lit(6)]));
    }

    #[test]
    fn deep_root_normalisation_applies_partial_eval_steps() {
        let expr = root(vec![Expr::Or(
            Metadata::new(),
            Moo::new(matrix_expr![bool_lit(false), int_lit(1)]),
        )]);
        let normalised = normalise_root_constraints_deep(&expr).unwrap();
        assert_eq!(
            normalised,
            root(vec![Expr::Or(
                Metadata::new(),
                Moo::new(matrix_expr![int_lit(1)]),
            )])
        );
    }

    #[test]
    fn selective_deep_root_normalisation_skips_solver_flat_constraints() {
        let flat = Expr::FlatProductEq(
            Metadata::new(),
            Moo::new(Atom::Literal(Lit::Int(1))),
            Moo::new(Atom::Literal(Lit::Int(2))),
            Moo::new(Atom::Literal(Lit::Int(3))),
        );
        let expr = root(vec![bool_lit(true), flat]);
        assert!(normalise_root_constraints_deep(&expr).is_none());
    }

    #[test]
    fn deep_root_normalisation_terminates_on_already_folded_constraint() {
        let expr = root(vec![int_lit(5)]);
        assert!(normalise_root_constraints_deep(&expr).is_none());
    }

    #[test]
    fn local_evaluator_normalisation_terminates_on_already_folded_literal() {
        let expr = int_lit(5);
        assert!(normalise_evaluator_local(&expr).is_none());
    }

    #[test]
    fn constant_set_cardinality_is_folded() {
        let set = Expr::Atomic(
            Metadata::new(),
            Atom::Literal(Lit::AbstractLiteral(AbstractLiteral::Set(vec![
                Lit::Int(1),
                Lit::Int(2),
            ]))),
        );
        let cardinality = Expr::Card(Metadata::new(), Moo::new(set));

        assert_eq!(eval_constant(&cardinality), Some(Lit::Int(2)));
    }

    #[test]
    fn constant_set_minimum_and_maximum_are_folded() {
        let set = Expr::from(Lit::AbstractLiteral(AbstractLiteral::Set(vec![
            Lit::Int(4),
            Lit::Int(1),
            Lit::Int(3),
        ])));

        assert_eq!(
            eval_constant(&Expr::Min(Metadata::new(), Moo::new(set.clone()))),
            Some(Lit::Int(1))
        );
        assert_eq!(
            eval_constant(&Expr::Max(Metadata::new(), Moo::new(set))),
            Some(Lit::Int(4))
        );
    }

    #[test]
    fn constant_set_membership_supports_composite_values() {
        let member =
            Lit::AbstractLiteral(AbstractLiteral::Tuple(vec![Lit::Bool(true), Lit::Int(2)]));
        let set = Lit::AbstractLiteral(AbstractLiteral::Set(vec![member.clone()]));
        let membership = Expr::In(
            Metadata::new(),
            Moo::new(Expr::from(member)),
            Moo::new(Expr::from(set)),
        );

        assert_eq!(eval_constant(&membership), Some(Lit::Bool(true)));
    }

    #[test]
    fn constant_equality_supports_composite_values() {
        let tuple = Lit::AbstractLiteral(AbstractLiteral::Tuple(vec![Lit::Int(1), Lit::Int(3)]));
        let equal = Expr::Eq(
            Metadata::new(),
            Moo::new(Expr::from(tuple.clone())),
            Moo::new(Expr::from(tuple)),
        );
        let unequal = Expr::Neq(
            Metadata::new(),
            Moo::new(Expr::from(Lit::AbstractLiteral(AbstractLiteral::Tuple(
                vec![Lit::Int(1), Lit::Int(3)],
            )))),
            Moo::new(Expr::from(Lit::AbstractLiteral(AbstractLiteral::Tuple(
                vec![Lit::Int(1), Lit::Int(4)],
            )))),
        );

        assert_eq!(eval_constant(&equal), Some(Lit::Bool(true)));
        assert_eq!(eval_constant(&unequal), Some(Lit::Bool(true)));

        let set_equality = Expr::Eq(
            Metadata::new(),
            Moo::new(Expr::from(Lit::AbstractLiteral(AbstractLiteral::Set(
                vec![Lit::Int(1), Lit::Int(2)],
            )))),
            Moo::new(Expr::from(Lit::AbstractLiteral(AbstractLiteral::Set(
                vec![Lit::Int(2), Lit::Int(1)],
            )))),
        );
        assert_eq!(eval_constant(&set_equality), Some(Lit::Bool(true)));
    }

    #[test]
    fn finish_root_normalisation_applies_local_root_rules() {
        let expr = root(vec![bool_lit(true), int_lit(1)]);
        let normalised = finish_root_evaluator_normalisation(&expr).unwrap();
        assert_eq!(normalised, root(vec![int_lit(1)]));
    }

    #[test]
    fn selective_deep_only_constraint_folds_target_index() {
        let ground = Expr::Sum(
            Metadata::new(),
            Moo::new(matrix_expr![int_lit(1), int_lit(2), int_lit(3)]),
        );
        let untouched = Expr::Sum(
            Metadata::new(),
            Moo::new(matrix_expr![int_lit(4), int_lit(5)]),
        );
        let expr = root(vec![ground, untouched.clone()]);
        let normalised = normalise_root_selective_deep_expr(&expr, Some(0)).unwrap();
        assert_eq!(normalised, root(vec![int_lit(6), untouched]));
    }

    #[test]
    fn finish_root_normalisation_flattens_top_level_and() {
        let expr = root(vec![Expr::And(
            Metadata::new(),
            Moo::new(matrix_expr![bool_lit(true), int_lit(1), int_lit(2)]),
        )]);
        let normalised = finish_root_evaluator_normalisation(&expr).unwrap();
        assert_eq!(normalised, root(vec![int_lit(1), int_lit(2)]));
    }
}
