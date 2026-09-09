use std::collections::HashSet;

use indexmap::IndexMap;

use crate::ast::Typeable;
use crate::{
    ast::{
        AbstractLiteral, Atom, DomainPtr, Expression as Expr, GroundDomain, Literal as Lit,
        Metadata, Moo, Range, ReturnType,
    },
    into_matrix_expr,
    rule_engine::{ApplicationError::RuleNotApplicable, ApplicationResult, RuleEffect},
};

/// Constant comparison shape used when dominating bounds under `And`.
///
/// Only same-operator bounds on the same atomic LHS are merged. Integer strictness is left alone
/// (`x > k` is not rewritten to `x >= k+1`) so later solver-family normalisers stay in control.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash)]
enum ConstantBoundOp {
    /// Keep the largest RHS: `x > a /\ x > b` ~> `x > max(a, b)`.
    Gt,
    /// Keep the largest RHS: `x >= a /\ x >= b` ~> `x >= max(a, b)`.
    Geq,
    /// Keep the smallest RHS: `x < a /\ x < b` ~> `x < min(a, b)`.
    Lt,
    /// Keep the smallest RHS: `x <= a /\ x <= b` ~> `x <= min(a, b)`.
    Leq,
}

impl ConstantBoundOp {
    /// Whether a larger RHS is the stronger bound for this operator.
    fn prefers_larger_rhs(self) -> bool {
        matches!(self, ConstantBoundOp::Gt | ConstantBoundOp::Geq)
    }
}

/// Normalises integer ranges so equivalent domains compare structurally equal.
fn normalise_int_domain(domain: &GroundDomain) -> GroundDomain {
    match domain {
        GroundDomain::Int(ranges) => GroundDomain::Int(Range::squeeze(
            &ranges
                .iter()
                .map(|range| Range::new(range.low().copied(), range.high().copied()))
                .collect::<Vec<_>>(),
        )),
        _ => domain.clone(),
    }
}

/// Returns whether `expr` is safe after resolving any referenced expressions.
fn is_semantically_safe(expr: &Expr) -> bool {
    fn helper(expr: &Expr, resolving: &mut HashSet<crate::ast::serde::ObjId>) -> bool {
        if matches!(
            expr,
            Expr::UnsafeDiv(_, _, _)
                | Expr::UnsafeMod(_, _, _)
                | Expr::UnsafePow(_, _, _)
                | Expr::UnsafeIndex(_, _, _)
                | Expr::Bubble(_, _, _)
                | Expr::UnsafeSlice(_, _, _)
        ) {
            return false;
        }

        if let Expr::Atomic(_, Atom::Reference(reference)) = expr {
            let id = reference.id();
            if !resolving.insert(id.clone()) {
                return false;
            }
            let is_safe = reference
                .with_resolved_expression(|resolved| helper(resolved, resolving))
                .unwrap_or(true);
            resolving.remove(&id);
            return is_safe;
        }

        let mut is_safe = true;
        expr.for_each_expr_child(&mut |child| {
            if is_safe {
                is_safe = helper(child, resolving);
            }
        });
        is_safe
    }

    helper(expr, &mut HashSet::new())
}

/// Tries to decide `expr in domain` from resolved domains alone.
fn simplify_in_domain(expr: &Expr, domain: &DomainPtr) -> Option<bool> {
    if !is_semantically_safe(expr) {
        return None;
    }

    let expr_domain = resolved_ground_domain_of_for_partial_eval(expr)?;
    let domain = domain.resolve().ok()?;
    let intersection = expr_domain.intersect(&domain).ok()?;

    if normalise_int_domain(&intersection) == normalise_int_domain(expr_domain.as_ref()) {
        return Some(true);
    }

    if let Ok(values_in_domain) = intersection.values_i32()
        && values_in_domain.is_empty()
    {
        return Some(false);
    }

    None
}

/// Extracts an integer when `expr` is known to be a singleton integer value.
fn singleton_int_value(expr: &Expr) -> Option<i32> {
    if let Ok(value) = expr.try_into() {
        return Some(value);
    }

    let domain = resolved_ground_domain_of_for_partial_eval(expr)?;
    let GroundDomain::Int(ranges) = domain.as_ref() else {
        return None;
    };
    let [range] = ranges.as_slice() else {
        return None;
    };
    let (Some(low), Some(high)) = (range.low(), range.high()) else {
        return None;
    };

    if low == high { Some(*low) } else { None }
}

/// Extracts a singleton integer without deriving the domain of a compound expression.
///
/// Local partial evaluation runs on every dirty ancestor. Calling `domain_of` for arithmetic
/// expressions here can enumerate the Cartesian product of both operand domains, turning a cheap
/// node-local check into the dominant translation cost. Literal and declaration-domain lookup are
/// the constant-size cases needed to fold operations such as `x ** 2` for `x : int(2..2)`.
fn cheap_singleton_int_value(expr: &Expr) -> Option<i32> {
    match expr {
        Expr::Atomic(_, Atom::Literal(Lit::Int(value))) => Some(*value),
        Expr::Atomic(_, Atom::Reference(reference)) => {
            let domain = reference.domain()?.resolve().ok()?;
            let GroundDomain::Int(ranges) = domain.as_ref() else {
                return None;
            };
            let [range] = ranges.as_slice() else {
                return None;
            };
            let (Some(low), Some(high)) = (range.low(), range.high()) else {
                return None;
            };
            (low == high).then_some(*low)
        }
        _ => None,
    }
}

fn matrix_index_offset(index_domain: &DomainPtr, index: i32) -> Option<usize> {
    let ranges = index_domain.as_int_ground()?;
    let [range] = ranges.as_slice() else {
        return None;
    };
    let from = *range.low()?;
    usize::try_from(index.checked_sub(from)?).ok()
}

fn ground_matrix_index_offset(index_domain: &GroundDomain, index: i32) -> Option<usize> {
    let GroundDomain::Int(ranges) = index_domain else {
        return None;
    };
    let [range] = ranges.as_slice() else {
        return None;
    };
    let from = *range.low()?;
    usize::try_from(index.checked_sub(from)?).ok()
}

/// Selects one element from a matrix literal, including a referenced constant matrix.
///
/// This deliberately clones only the selected element. Resolving the complete matrix into an
/// owned `Vec` for every index makes N selections from an N-element matrix quadratic.
fn resolve_matrix_element(subject: &Expr, index: i32) -> Option<Expr> {
    match subject {
        Expr::TypeAnnotation(_, inner, _) | Expr::DomainAnnotation(_, inner, _) => {
            resolve_matrix_element(inner, index)
        }
        Expr::AbstractLiteral(_, AbstractLiteral::Matrix(elems, index_domain)) => elems
            .get(matrix_index_offset(index_domain, index)?)
            .cloned(),
        Expr::Atomic(
            _,
            Atom::Literal(Lit::AbstractLiteral(AbstractLiteral::Matrix(elems, index_domain))),
        ) => elems
            .get(ground_matrix_index_offset(index_domain, index)?)
            .cloned()
            .map(|literal| Expr::Atomic(Metadata::new(), Atom::Literal(literal))),
        Expr::Atomic(_, Atom::Reference(reference)) => reference
            .with_resolved_expression(|resolved| resolve_matrix_element(resolved, index))
            .flatten()
            .or_else(|| {
                // Computed constant lettings are uncommon, but retain support for them. This
                // fallback may materialise the value; direct matrix lettings above do not.
                let Lit::AbstractLiteral(AbstractLiteral::Matrix(elems, index_domain)) =
                    reference.resolve_constant()?
                else {
                    return None;
                };
                elems
                    .get(ground_matrix_index_offset(&index_domain, index)?)
                    .cloned()
                    .map(|literal| Expr::Atomic(Metadata::new(), Atom::Literal(literal)))
            }),
        _ => None,
    }
}

/// Resolves domains for partial evaluation while avoiding malformed indexing panics.
fn resolved_ground_domain_of_for_partial_eval(expr: &Expr) -> Option<Moo<GroundDomain>> {
    match expr {
        Expr::SafeIndex(_, subject, _) => {
            let subject_domain = resolved_ground_domain_of_for_partial_eval(subject)?;
            let GroundDomain::Matrix(elem_domain, _) = subject_domain.as_ref() else {
                return None;
            };

            Some(elem_domain.clone())
        }
        Expr::SafeSlice(_, subject, indices) => {
            let subject_domain = resolved_ground_domain_of_for_partial_eval(subject)?;
            let GroundDomain::Matrix(elem_domain, index_domains) = subject_domain.as_ref() else {
                return None;
            };
            let sliced_dimension = indices.iter().position(Option::is_none);

            match sliced_dimension {
                Some(dimension) => Some(Moo::new(GroundDomain::Matrix(
                    elem_domain.clone(),
                    vec![index_domains[dimension].clone()],
                ))),
                None => Some(elem_domain.clone()),
            }
        }
        Expr::UnsafeIndex(_, _, _) | Expr::UnsafeSlice(_, _, _) => None,
        _ => expr.domain_of()?.resolve().ok(),
    }
}

/// Whether `expr` is an undefined value whose domain is empty.
///
/// `min`/`max` of an empty collection is the only unsafe shape that carries an empty domain, so it
/// is the only one for which [`simplify_comparison_with_literal`] derives a domain despite the
/// expression not being semantically safe.
fn is_empty_min_max(expr: &Expr) -> bool {
    matches!(expr, Expr::Min(_, values) | Expr::Max(_, values) if is_empty_matrix_operand(values))
}

/// Tries to decide `expr = lit` and `expr != lit` from the resolved domain of `expr`.
fn simplify_comparison_with_literal(expr: &Expr, lit: &Lit) -> Option<(bool, bool)> {
    // Deriving the domain of a compound expression enumerates its value set, and combining
    // operands takes the Cartesian product of those sets -- packed set representations reach tens
    // of thousands of values. Nothing below this point can decide the comparison for an
    // unsafe expression other than the empty `min`/`max` case, so bail out before paying for it.
    let is_safe = is_semantically_safe(expr);
    if !is_safe && !is_empty_min_max(expr) {
        return None;
    }

    let expr_domain = resolved_ground_domain_of_for_partial_eval(expr)?;

    // An empty domain represents an undefined value. Under relational semantics the closest
    // containing Boolean expression is false, for both equality and disequality.
    if matches!(expr_domain.as_ref(), GroundDomain::Empty(_)) {
        return Some((false, false));
    }

    if !is_safe {
        return None;
    }

    if !expr_domain.contains(lit).ok()? {
        // A representation rewrite can temporarily leave one side in its source shape and the
        // other in its represented shape (for example, `record = tuple`). The source domain does
        // not contain represented literals, but that does not make the original comparison false.
        if let Expr::Atomic(_, Atom::Reference(reference)) = expr
            && !reference.ptr().reprs().is_empty()
        {
            return None;
        }
        return Some((false, true));
    }

    match (expr_domain.as_ref(), lit) {
        (GroundDomain::Bool, Lit::Bool(_)) => None,
        (GroundDomain::Int(ranges), Lit::Int(value)) => {
            let [range] = ranges.as_slice() else {
                return None;
            };
            let (Some(low), Some(high)) = (range.low(), range.high()) else {
                return None;
            };

            if low == high && low == value {
                Some((true, false))
            } else {
                None
            }
        }
        _ => None,
    }
}

/// Whether deciding a comparison against `expr` is worth a domain lookup in this mode.
///
/// The local evaluator runs on every dirty ancestor after every rewrite, so it only derives a
/// domain when that is a constant-size lookup: a declaration's own domain, or the empty domain of
/// an empty `min`/`max`. Deep evaluation runs once per expansion and can afford the general case.
fn comparison_domain_lookup_is_cheap(expr: &Expr, mode: PartialEvalMode) -> bool {
    mode == PartialEvalMode::Deep
        || matches!(expr, Expr::Atomic(_, Atom::Reference(_)))
        || is_empty_min_max(expr)
}

/// Tries to decide reflexive equality and inequality when both sides are semantically safe.
fn simplify_reflexive_comparison(x: &Expr, y: &Expr) -> Option<(bool, bool)> {
    if x.identical_atom_to(y) && is_semantically_safe(x) && is_semantically_safe(y) {
        return Some((true, false));
    }

    if is_semantically_safe(x) && is_semantically_safe(y) && x == y {
        return Some((true, false));
    }

    None
}

fn simplify_reflexive_comparison_with_mode(
    x: &Expr,
    y: &Expr,
    mode: PartialEvalMode,
) -> Option<(bool, bool)> {
    match mode {
        PartialEvalMode::Deep => simplify_reflexive_comparison(x, y),
        PartialEvalMode::Local => x.identical_atom_to(y).then_some((true, false)),
    }
}

pub fn run_partial_evaluator(expr: &Expr) -> ApplicationResult {
    run_partial_evaluator_with_mode(expr, PartialEvalMode::Deep)
}

/// Partially evaluates `expr` using only information already available at the focused node.
///
/// This is intended for the main rewriter, where recursive simplification is supplied by the
/// scheduler. Use [`run_partial_evaluator`] when a caller explicitly wants semantic checks that
/// can inspect referenced expressions.
pub fn run_partial_evaluator_local(expr: &Expr) -> ApplicationResult {
    run_partial_evaluator_with_mode(expr, PartialEvalMode::Local)
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
enum PartialEvalMode {
    Deep,
    Local,
}

/// Rewrites `x = true` / `true = x` to `x` when `x` is a non-literal boolean atom.
///
/// Nested uses such as `(x = true) <-> and([y = true, ...])` otherwise force Minion flattening to
/// introduce aux variables for the left-hand `Eq`, because both sides of the outer equality are
/// non-atomic. Lowering the tautological `= true` first keeps a boolean decision variable atomic
/// so later equality/reify rules can target it directly.
///
/// Literal–literal equalities are left alone for constant folding. Non-boolean atoms are refused.
pub fn try_lower_bool_atom_eq_true(expr: &Expr) -> Option<Expr> {
    let Expr::Eq(_, left, right) = expr else {
        return None;
    };

    let atom = match (left.as_ref(), right.as_ref()) {
        (Expr::Atomic(_, Atom::Literal(Lit::Bool(true))), Expr::Atomic(_, atom))
            if !matches!(atom, Atom::Literal(_)) =>
        {
            right.as_ref()
        }
        (Expr::Atomic(_, atom), Expr::Atomic(_, Atom::Literal(Lit::Bool(true))))
            if !matches!(atom, Atom::Literal(_)) =>
        {
            left.as_ref()
        }
        _ => return None,
    };

    if atom.return_type() != ReturnType::Bool {
        return None;
    }

    Some(atom.clone())
}

fn run_partial_evaluator_with_mode(expr: &Expr, mode: PartialEvalMode) -> ApplicationResult {
    // NOTE: If nothing changes, we must return RuleNotApplicable, or the rewriter will try this
    // rule infinitely!
    // This is why we always check whether we found a constant or not.
    match expr {
        Expr::Union(_, _, _) => Err(RuleNotApplicable),
        Expr::In(_, _, _) => Err(RuleNotApplicable),
        Expr::Intersect(_, _, _) => Err(RuleNotApplicable),
        Expr::Supset(_, _, _) => Err(RuleNotApplicable),
        Expr::SupsetEq(_, _, _) => Err(RuleNotApplicable),
        Expr::Subset(_, _, _) => Err(RuleNotApplicable),
        Expr::SubsetEq(_, _, _) => Err(RuleNotApplicable),
        Expr::AbstractLiteral(_, _) => Err(RuleNotApplicable),
        Expr::Comprehension(_, _) => Err(RuleNotApplicable),
        Expr::AbstractComprehension(_, _) => Err(RuleNotApplicable),
        Expr::DominanceRelation(_, _) => Err(RuleNotApplicable),
        Expr::TypeAnnotation(_, _, _) => Err(RuleNotApplicable),
        Expr::DomainAnnotation(_, _, _) => Err(RuleNotApplicable),
        Expr::FromSolution(_, _) => Err(RuleNotApplicable),
        Expr::Metavar(_, _) => Err(RuleNotApplicable),
        Expr::UnsafeIndex(_, _, _) => Err(RuleNotApplicable),
        Expr::UnsafeSlice(_, _, _) => Err(RuleNotApplicable),
        Expr::Table(_, _, _) => Err(RuleNotApplicable),
        Expr::NegativeTable(_, _, _) => Err(RuleNotApplicable),
        Expr::AtLeast(_, _, _, _) => Err(RuleNotApplicable),
        Expr::AtMost(_, _, _, _) => Err(RuleNotApplicable),
        Expr::Gcc(_, _, _, _) | Expr::GccWeak(_, _, _, _) => Err(RuleNotApplicable),
        Expr::RecordField(_, _, _) => Err(RuleNotApplicable),
        Expr::AttributeAsConstraint(_, _, _, _) => Err(RuleNotApplicable),
        Expr::SafeIndex(_, subject, indices) => {
            // partially evaluate matrix literals indexed by a constant.
            if indices.is_empty() {
                return Err(RuleNotApplicable);
            }

            // the leading index must be fixed to a single value
            let index = singleton_int_value(&indices[0]).ok_or(RuleNotApplicable)?;

            let selected = resolve_matrix_element(subject, index).ok_or(RuleNotApplicable)?;
            if indices.len() == 1 {
                Ok(RuleEffect::pure(selected))
            } else {
                Ok(RuleEffect::pure(Expr::SafeIndex(
                    Metadata::new(),
                    Moo::new(selected),
                    indices[1..].to_vec(),
                )))
            }
        }
        Expr::SafeSlice(_, _, _) => Err(RuleNotApplicable),
        Expr::InDomain(_, x, domain) => {
            if mode == PartialEvalMode::Deep
                && let Some(result) = simplify_in_domain(x, domain)
            {
                Ok(RuleEffect::pure(Expr::Atomic(
                    Metadata::new(),
                    result.into(),
                )))
            } else if let Expr::Atomic(_, Atom::Reference(decl)) = x.as_ref() {
                let decl_domain = decl
                    .domain()
                    .ok_or(RuleNotApplicable)?
                    .resolve()
                    .map_err(|_| RuleNotApplicable)?;
                let domain = domain.resolve().map_err(|_| RuleNotApplicable)?;

                let intersection = decl_domain
                    .intersect(&domain)
                    .map_err(|_| RuleNotApplicable)?;

                // if the declaration's domain is a subset of domain, expr is always true.
                if &intersection == decl_domain.as_ref() {
                    Ok(RuleEffect::pure(Expr::Atomic(Metadata::new(), true.into())))
                }
                // if no elements of declaration's domain are in the domain (i.e. they have no
                // intersection), expr is always false.
                //
                // Only check this when the intersection is a finite integer domain, as we
                // currently don't have a way to check whether other domain kinds are empty or not.
                //
                // we should expand this to cover more domain types in the future.
                else if let Ok(values_in_domain) = intersection.values_i32()
                    && values_in_domain.is_empty()
                {
                    Ok(RuleEffect::pure(Expr::Atomic(
                        Metadata::new(),
                        false.into(),
                    )))
                } else {
                    Err(RuleNotApplicable)
                }
            } else if let Expr::Atomic(_, Atom::Literal(lit)) = x.as_ref() {
                if domain
                    .resolve()
                    .and_then(|gd| gd.contains(lit))
                    .map_err(|_| RuleNotApplicable)?
                {
                    Ok(RuleEffect::pure(Expr::Atomic(Metadata::new(), true.into())))
                } else {
                    Ok(RuleEffect::pure(Expr::Atomic(
                        Metadata::new(),
                        false.into(),
                    )))
                }
            } else {
                Err(RuleNotApplicable)
            }
        }
        Expr::Bubble(_, expr, cond) => {
            // definition of bubble is "expr is valid as long as cond is true"
            //
            // check if cond is true and pop the bubble!
            if let Expr::Atomic(_, Atom::Literal(Lit::Bool(true))) = cond.as_ref() {
                Ok(RuleEffect::pure(Moo::unwrap_or_clone(expr.clone())))
            } else {
                Err(RuleNotApplicable)
            }
        }
        Expr::Atomic(_, _) => Err(RuleNotApplicable),
        Expr::ToInt(_, expression) => {
            if expression.return_type() == ReturnType::Int {
                Ok(RuleEffect::pure(Moo::unwrap_or_clone(expression.clone())))
            } else {
                Err(RuleNotApplicable)
            }
        }
        Expr::Abs(m, e) => match e.as_ref() {
            Expr::Neg(_, inner) => Ok(RuleEffect::pure(Expr::Abs(m.clone(), inner.clone()))),
            _ => Err(RuleNotApplicable),
        },
        Expr::Sum(m, vec) => {
            let vec = vec.unwrap_list_cow().ok_or(RuleNotApplicable)?;
            let mut acc = 0;
            let mut n_consts = 0;
            for expr in vec.iter() {
                if let Expr::Atomic(_, Atom::Literal(Lit::Int(x))) = expr {
                    acc += *x;
                    n_consts += 1;
                }
            }

            if n_consts <= 1 {
                return Err(RuleNotApplicable);
            }

            let mut new_vec: Vec<Expr> = vec
                .iter()
                .filter(|expr| !matches!(expr, Expr::Atomic(_, Atom::Literal(Lit::Int(_)))))
                .cloned()
                .collect();
            if acc != 0 {
                new_vec.push(Expr::Atomic(
                    Default::default(),
                    Atom::Literal(Lit::Int(acc)),
                ));
            }

            Ok(RuleEffect::pure(Expr::Sum(
                m.clone(),
                Moo::new(into_matrix_expr![new_vec]),
            )))
        }

        Expr::Product(m, vec) => {
            let mut acc = 1;
            let mut n_consts = 0;
            let vec = vec.unwrap_list_cow().ok_or(RuleNotApplicable)?;
            for expr in vec.iter() {
                if let Expr::Atomic(_, Atom::Literal(Lit::Int(x))) = expr {
                    acc *= *x;
                    n_consts += 1;
                }
            }

            if n_consts == 0 {
                return Err(RuleNotApplicable);
            }

            if acc == 0 && mode == PartialEvalMode::Local {
                return Err(RuleNotApplicable);
            }

            if acc != 0 && n_consts == 1 {
                return Err(RuleNotApplicable);
            }

            let mut new_vec: Vec<Expr> = vec
                .iter()
                .filter(|expr| !matches!(expr, Expr::Atomic(_, Atom::Literal(Lit::Int(_)))))
                .cloned()
                .collect();

            new_vec.push(Expr::Atomic(
                Default::default(),
                Atom::Literal(Lit::Int(acc)),
            ));
            let new_product = Expr::Product(m.clone(), Moo::new(into_matrix_expr![new_vec]));

            if acc == 0 {
                // If safe, 0 * exprs ~> 0. Otherwise do not reshuffle: appending the folded
                // zero fights `reorder_product` (constant-first) and loops forever under the
                // local evaluator. Constant folding/placement is left to `reorder_product`.
                if mode == PartialEvalMode::Deep && is_semantically_safe(&new_product) {
                    Ok(RuleEffect::pure(Expr::Atomic(
                        Default::default(),
                        Atom::Literal(Lit::Int(0)),
                    )))
                } else {
                    Err(RuleNotApplicable)
                }
            } else {
                // acc !=0, multiple constants found
                Ok(RuleEffect::pure(new_product))
            }
        }

        Expr::Min(m, e) => {
            let Some(vec) = e.unwrap_list_cow() else {
                return Err(RuleNotApplicable);
            };
            let mut acc: Option<i32> = None;
            let mut n_consts = 0;
            for expr in vec.iter() {
                if let Expr::Atomic(_, Atom::Literal(Lit::Int(x))) = expr {
                    n_consts += 1;
                    acc = match acc {
                        Some(i) => {
                            if i > *x {
                                Some(*x)
                            } else {
                                Some(i)
                            }
                        }
                        None => Some(*x),
                    };
                }
            }

            if n_consts <= 1 {
                return Err(RuleNotApplicable);
            }

            let mut new_vec: Vec<Expr> = vec
                .iter()
                .filter(|expr| !matches!(expr, Expr::Atomic(_, Atom::Literal(Lit::Int(_)))))
                .cloned()
                .collect();
            if let Some(i) = acc {
                new_vec.push(Expr::Atomic(Default::default(), Atom::Literal(Lit::Int(i))));
            }

            Ok(RuleEffect::pure(Expr::Min(
                m.clone(),
                Moo::new(into_matrix_expr![new_vec]),
            )))
        }

        Expr::Max(m, e) => {
            let Some(vec) = e.unwrap_list_cow() else {
                return Err(RuleNotApplicable);
            };

            let mut acc: Option<i32> = None;
            let mut n_consts = 0;
            for expr in vec.iter() {
                if let Expr::Atomic(_, Atom::Literal(Lit::Int(x))) = expr {
                    n_consts += 1;
                    acc = match acc {
                        Some(i) => {
                            if i < *x {
                                Some(*x)
                            } else {
                                Some(i)
                            }
                        }
                        None => Some(*x),
                    };
                }
            }

            if n_consts <= 1 {
                return Err(RuleNotApplicable);
            }

            let mut new_vec: Vec<Expr> = vec
                .iter()
                .filter(|expr| !matches!(expr, Expr::Atomic(_, Atom::Literal(Lit::Int(_)))))
                .cloned()
                .collect();
            if let Some(i) = acc {
                new_vec.push(Expr::Atomic(Default::default(), Atom::Literal(Lit::Int(i))));
            }

            Ok(RuleEffect::pure(Expr::Max(
                m.clone(),
                Moo::new(into_matrix_expr![new_vec]),
            )))
        }
        Expr::Not(_, e1) => {
            let Expr::Imply(_, p, q) = e1.as_ref() else {
                return Err(RuleNotApplicable);
            };

            if mode == PartialEvalMode::Deep && !is_semantically_safe(e1) {
                return Err(RuleNotApplicable);
            }

            match (p.as_ref(), q.as_ref()) {
                (_, Expr::Atomic(_, Atom::Literal(Lit::Bool(true)))) => {
                    Ok(RuleEffect::pure(Expr::from(false)))
                }
                (_, Expr::Atomic(_, Atom::Literal(Lit::Bool(false)))) => {
                    Ok(RuleEffect::pure(Moo::unwrap_or_clone(p.clone())))
                }
                (Expr::Atomic(_, Atom::Literal(Lit::Bool(true))), _) => {
                    Ok(RuleEffect::pure(Expr::Not(Metadata::new(), q.clone())))
                }
                (Expr::Atomic(_, Atom::Literal(Lit::Bool(false))), _) => {
                    Ok(RuleEffect::pure(Expr::from(false)))
                }
                _ => Err(RuleNotApplicable),
            }
        }
        Expr::Or(m, e) => {
            // Empty disjunction is the Or-identity, whatever index domain the matrix carries.
            if is_empty_matrix_operand(e) {
                return Ok(RuleEffect::pure(Expr::from(false)));
            }

            let Some(terms) = e.unwrap_list_cow() else {
                return Err(RuleNotApplicable);
            };

            let mut has_changed = false;
            let mut non_literal_terms = 0;

            // Inspection pass: decide applicability from borrowed terms, so a disjunction that
            // does not fold costs a walk rather than a copy.
            for expr in terms.iter() {
                if let Expr::Atomic(_, Atom::Literal(Lit::Bool(x))) = expr {
                    has_changed = true;

                    // true ~~> entire or is true
                    // false ~~> remove false from the or
                    if *x {
                        return Ok(RuleEffect::pure(true.into()));
                    }
                } else {
                    non_literal_terms += 1;
                }
            }

            // The two supported implication tautologies, in expected O(n).
            if check_pairwise_or_tautologies(&terms) {
                return Ok(RuleEffect::pure(true.into()));
            }

            // 3. empty or ~~> false
            if non_literal_terms == 0 {
                return Ok(RuleEffect::pure(false.into()));
            }

            if !has_changed {
                return Err(RuleNotApplicable);
            }

            let new_terms = terms
                .iter()
                .filter(|expr| !matches!(expr, Expr::Atomic(_, Atom::Literal(Lit::Bool(_)))))
                .cloned()
                .collect::<Vec<_>>();

            Ok(RuleEffect::pure(Expr::Or(
                m.clone(),
                Moo::new(into_matrix_expr![new_terms]),
            )))
        }
        Expr::And(_, e) => {
            // The evaluator revisits a conjunction after each rewrite elsewhere in the model, so
            // establish applicability from borrowed conjuncts and only build a replacement once
            // the rule is known to apply. A wide `and` -- what unrolling `forAll i : D. ...`
            // produces -- is then walked per visit rather than copied, keeping repeated visits
            // linear rather than quadratic in the number of conjuncts.
            // Empty conjunction is the And-identity, whatever index domain the matrix carries.
            if is_empty_matrix_operand(e) {
                return Ok(RuleEffect::pure(Expr::from(true)));
            }

            let Some(vec) = e.unwrap_list_cow() else {
                return Err(RuleNotApplicable);
            };

            let mut has_changed: bool = false;
            // `Atom` is only interior-mutable through `DeclarationPtr`, whose `Hash`/`Eq` use the
            // immutable declaration id, so it is stable as a key.
            #[allow(clippy::mutable_key_type)]
            let mut distinct_bounds: HashSet<(&Atom, ConstantBoundOp)> = HashSet::new();
            let mut constant_bound_terms: usize = 0;
            for expr in vec.iter() {
                if let Expr::Atomic(_, Atom::Literal(Lit::Bool(x))) = expr {
                    if !x {
                        return Ok(RuleEffect::pure(Expr::Atomic(
                            Default::default(),
                            Atom::Literal(Lit::Bool(false)),
                        )));
                    }
                    has_changed = true;
                } else if let Some((lhs, op, _)) = as_constant_bound_comparison_ref(expr) {
                    constant_bound_terms += 1;
                    distinct_bounds.insert((lhs, op));
                }
            }

            // Only treat bound aggregation as a change when at least one conjunct was dominated.
            if constant_bound_terms > distinct_bounds.len() {
                has_changed = true;
            }

            if !has_changed {
                return Err(RuleNotApplicable);
            }
            drop(distinct_bounds);

            // The rule applies: now build the replacement.
            let mut new_vec: Vec<Expr> = Vec::new();
            // Strongest constant bound per (atomic LHS, comparison operator), first-seen order.
            let mut constant_bounds: IndexMap<(Atom, ConstantBoundOp), i32> = IndexMap::new();
            for expr in vec.iter() {
                if matches!(expr, Expr::Atomic(_, Atom::Literal(Lit::Bool(_)))) {
                    continue;
                } else if let Some((lhs, op, rhs)) = as_constant_bound_comparison(expr) {
                    merge_constant_bound(&mut constant_bounds, lhs, op, rhs);
                } else {
                    new_vec.push(expr.clone());
                }
            }

            for ((lhs, op), rhs) in constant_bounds {
                new_vec.push(make_constant_bound_comparison(lhs, op, rhs));
            }

            if new_vec.is_empty() {
                Ok(RuleEffect::pure(Expr::from(true)))
            } else {
                Ok(RuleEffect::pure(Expr::And(
                    Metadata::new(),
                    Moo::new(into_matrix_expr![new_vec]),
                )))
            }
        }

        // similar to And, but booleans are returned wrapped in Root.
        Expr::Root(_, es) => {
            match es.as_slice() {
                [] => Err(RuleNotApplicable),
                // want to unwrap nested ands
                [Expr::And(_, _)] => Ok(()),
                // root([true]) / root([false]) are already evaluated
                [_] => Err(RuleNotApplicable),
                [_, _, ..] => Ok(()),
            }?;

            let mut new_vec: Vec<Expr> = Vec::new();
            let mut has_changed: bool = false;
            for expr in es {
                match expr {
                    Expr::Atomic(_, Atom::Literal(Lit::Bool(x))) => {
                        has_changed = true;
                        if !x {
                            // false
                            return Ok(RuleEffect::pure(Expr::Root(
                                Metadata::new(),
                                vec![Expr::Atomic(
                                    Default::default(),
                                    Atom::Literal(Lit::Bool(false)),
                                )],
                            )));
                        }
                        // remove trues
                    }

                    // flatten ands in root, applying the same true/false rules to conjuncts
                    Expr::And(_, vecs) => match Moo::unwrap_or_clone(vecs.clone()).unwrap_list() {
                        Some(list) => {
                            has_changed = true;
                            for conjunct in list {
                                match conjunct {
                                    Expr::Atomic(_, Atom::Literal(Lit::Bool(false))) => {
                                        return Ok(RuleEffect::pure(Expr::Root(
                                            Metadata::new(),
                                            vec![Expr::Atomic(
                                                Default::default(),
                                                Atom::Literal(Lit::Bool(false)),
                                            )],
                                        )));
                                    }
                                    Expr::Atomic(_, Atom::Literal(Lit::Bool(true))) => {}
                                    other => new_vec.push(other),
                                }
                            }
                        }
                        None => new_vec.push(expr.clone()),
                    },
                    _ => new_vec.push(expr.clone()),
                }
            }

            if !has_changed {
                Err(RuleNotApplicable)
            } else {
                if new_vec.is_empty() {
                    new_vec.push(true.into());
                }
                Ok(RuleEffect::pure(Expr::Root(Metadata::new(), new_vec)))
            }
        }
        Expr::Imply(_m, x, y) => {
            if let Expr::Atomic(_, Atom::Literal(Lit::Bool(x))) = x.as_ref() {
                return if *x {
                    // (true) -> y ~~> y
                    Ok(RuleEffect::pure(Moo::unwrap_or_clone(y.clone())))
                } else {
                    // (false) -> y ~~> true
                    Ok(RuleEffect::pure(Expr::Atomic(Metadata::new(), true.into())))
                };
            };

            if let Expr::Atomic(_, Atom::Literal(Lit::Bool(y))) = y.as_ref() {
                return if *y {
                    // x -> (true) ~~> true
                    Ok(RuleEffect::pure(Expr::from(true)))
                } else {
                    // x -> (false) ~~> !x
                    Ok(RuleEffect::pure(Expr::Not(Metadata::new(), x.clone())))
                };
            };

            // reflexivity: p -> p ~> true

            // instead of checking syntactic equivalence of a possibly deep expression,
            // let identical-CSE turn them into identical variables first. Then, check if they are
            // identical variables.

            if x.identical_atom_to(y.as_ref())
                && (mode == PartialEvalMode::Local
                    || (is_semantically_safe(x) && is_semantically_safe(y)))
            {
                return Ok(RuleEffect::pure(true.into()));
            }

            Err(RuleNotApplicable)
        }
        Expr::Iff(_m, x, y) => {
            if let Expr::Atomic(_, Atom::Literal(Lit::Bool(x))) = x.as_ref() {
                return if *x {
                    // (true) <-> y ~~> y
                    Ok(RuleEffect::pure(Moo::unwrap_or_clone(y.clone())))
                } else {
                    // (false) <-> y ~~> !y
                    Ok(RuleEffect::pure(Expr::Not(Metadata::new(), y.clone())))
                };
            };
            if let Expr::Atomic(_, Atom::Literal(Lit::Bool(y))) = y.as_ref() {
                return if *y {
                    // x <-> (true) ~~> x
                    Ok(RuleEffect::pure(Moo::unwrap_or_clone(x.clone())))
                } else {
                    // x <-> (false) ~~> !x
                    Ok(RuleEffect::pure(Expr::Not(Metadata::new(), x.clone())))
                };
            };

            // reflexivity: p <-> p ~> true

            // instead of checking syntactic equivalence of a possibly deep expression,
            // let identical-CSE turn them into identical variables first. Then, check if they are
            // identical variables.

            if x.identical_atom_to(y.as_ref())
                && (mode == PartialEvalMode::Local
                    || (is_semantically_safe(x) && is_semantically_safe(y)))
            {
                return Ok(RuleEffect::pure(true.into()));
            }

            Err(RuleNotApplicable)
        }
        Expr::Eq(_, x, y) => {
            if let Some((eq_result, _)) = simplify_reflexive_comparison_with_mode(x, y, mode) {
                Ok(RuleEffect::pure(Expr::Atomic(
                    Metadata::new(),
                    Atom::Literal(Lit::Bool(eq_result)),
                )))
            } else if let Expr::Atomic(_, Atom::Literal(lit)) = x.as_ref()
                && comparison_domain_lookup_is_cheap(y, mode)
                && let Some((eq_result, _)) = simplify_comparison_with_literal(y, lit)
            {
                Ok(RuleEffect::pure(Expr::Atomic(
                    Metadata::new(),
                    Atom::Literal(Lit::Bool(eq_result)),
                )))
            } else if let Expr::Atomic(_, Atom::Literal(lit)) = y.as_ref()
                && comparison_domain_lookup_is_cheap(x, mode)
                && let Some((eq_result, _)) = simplify_comparison_with_literal(x, lit)
            {
                Ok(RuleEffect::pure(Expr::Atomic(
                    Metadata::new(),
                    Atom::Literal(Lit::Bool(eq_result)),
                )))
            } else if let Some(atom) = try_lower_bool_atom_eq_true(expr) {
                Ok(RuleEffect::pure(atom))
            } else {
                Err(RuleNotApplicable)
            }
        }
        Expr::Neq(_, x, y) => {
            if let Some((_, neq_result)) = simplify_reflexive_comparison_with_mode(x, y, mode) {
                Ok(RuleEffect::pure(Expr::Atomic(
                    Metadata::new(),
                    Atom::Literal(Lit::Bool(neq_result)),
                )))
            } else if let Expr::Atomic(_, Atom::Literal(lit)) = x.as_ref()
                && comparison_domain_lookup_is_cheap(y, mode)
                && let Some((_, neq_result)) = simplify_comparison_with_literal(y, lit)
            {
                Ok(RuleEffect::pure(Expr::Atomic(
                    Metadata::new(),
                    Atom::Literal(Lit::Bool(neq_result)),
                )))
            } else if let Expr::Atomic(_, Atom::Literal(lit)) = y.as_ref()
                && comparison_domain_lookup_is_cheap(x, mode)
                && let Some((_, neq_result)) = simplify_comparison_with_literal(x, lit)
            {
                Ok(RuleEffect::pure(Expr::Atomic(
                    Metadata::new(),
                    Atom::Literal(Lit::Bool(neq_result)),
                )))
            } else {
                Err(RuleNotApplicable)
            }
        }
        Expr::Geq(_, _, _) => Err(RuleNotApplicable),
        Expr::Leq(_, _, _) => Err(RuleNotApplicable),
        Expr::Gt(_, _, _) => Err(RuleNotApplicable),
        Expr::Lt(_, _, _) => Err(RuleNotApplicable),
        Expr::SafeDiv(_, _, _) => Err(RuleNotApplicable),
        Expr::UnsafeDiv(_, _, _) => Err(RuleNotApplicable),
        Expr::Flatten(_, _, _) => Err(RuleNotApplicable), // TODO: check if anything can be done here
        Expr::AllDiff(m, e) => {
            let Some((vec, _)) = Moo::unwrap_or_clone(e.clone()).unwrap_matrix_unchecked() else {
                return Err(RuleNotApplicable);
            };

            let mut consts: HashSet<Lit> = HashSet::new();

            // A fully constant allDiff can be decided immediately.
            for expr in vec {
                let Expr::Atomic(_, Atom::Literal(lit)) = expr else {
                    return Err(RuleNotApplicable);
                };
                if !consts.insert(lit) {
                    return Ok(RuleEffect::pure(Expr::Atomic(
                        m.clone(),
                        Atom::Literal(Lit::Bool(false)),
                    )));
                }
            }

            Ok(RuleEffect::pure(Expr::Atomic(
                m.clone(),
                Atom::Literal(Lit::Bool(true)),
            )))
        }
        Expr::Neg(_, _) => Err(RuleNotApplicable),
        Expr::Factorial(_, _) => Err(RuleNotApplicable),
        Expr::AuxDeclaration(_, _, _) => Err(RuleNotApplicable),
        Expr::UnsafeMod(_, _, _) => Err(RuleNotApplicable),
        Expr::SafeMod(_, _, _) => Err(RuleNotApplicable),
        Expr::UnsafePow(_, _, _) => Err(RuleNotApplicable),
        Expr::SafePow(_, base, exponent) => {
            let base = cheap_singleton_int_value(base).ok_or(RuleNotApplicable)?;
            let exponent = cheap_singleton_int_value(exponent).ok_or(RuleNotApplicable)?;
            if exponent < 0 || (base == 0 && exponent == 0) {
                return Err(RuleNotApplicable);
            }
            let value = base.checked_pow(exponent as u32).ok_or(RuleNotApplicable)?;
            Ok(RuleEffect::pure(Expr::from(value)))
        }
        Expr::Minus(_, _, _) => Err(RuleNotApplicable),
        Expr::Card(_, _) => Err(RuleNotApplicable),

        // As these are in a low level solver form, I'm assuming that these have already been
        // simplified and partially evaluated.
        Expr::FlatAllDiff(_, _) => Err(RuleNotApplicable),
        Expr::SmtDistinct(_, _) => Err(RuleNotApplicable),
        Expr::FlatAbsEq(_, _, _) => Err(RuleNotApplicable),
        Expr::FlatMinEq(_, _, _) => Err(RuleNotApplicable),
        Expr::FlatIneq(_, _, _, _) => Err(RuleNotApplicable),
        Expr::FlatMinusEq(_, _, _) => Err(RuleNotApplicable),
        Expr::FlatProductEq(_, _, _, _) => Err(RuleNotApplicable),
        Expr::FlatSumLeq(_, _, _) => Err(RuleNotApplicable),
        Expr::FlatSumGeq(_, _, _) => Err(RuleNotApplicable),
        Expr::FlatWatchedLiteral(_, _, _) => Err(RuleNotApplicable),
        Expr::FlatWeightedSumLeq(_, _, _, _) => Err(RuleNotApplicable),
        Expr::FlatWeightedSumGeq(_, _, _, _) => Err(RuleNotApplicable),
        Expr::MinionDivEqUndefZero(_, _, _, _) => Err(RuleNotApplicable),
        Expr::MinionModuloEqUndefZero(_, _, _, _) => Err(RuleNotApplicable),
        Expr::MinionPow(_, _, _, _) => Err(RuleNotApplicable),
        Expr::MinionReify(_, _, _) => Err(RuleNotApplicable),
        Expr::MinionReifyImply(_, _, _) => Err(RuleNotApplicable),
        Expr::MinionWInIntervalSet(_, _, _) => Err(RuleNotApplicable),
        Expr::MinionWInSet(_, _, _) => Err(RuleNotApplicable),
        Expr::MinionElementOne(_, _, _, _) => Err(RuleNotApplicable),
        Expr::SATInt(_, _, _, _) => Err(RuleNotApplicable),
        Expr::PairwiseSum(_, _, _) => Err(RuleNotApplicable),
        Expr::PairwiseProduct(_, _, _) => Err(RuleNotApplicable),
        Expr::Active(m, variant, alternative) => {
            let active_alternative = match variant.as_ref() {
                Expr::AbstractLiteral(_, AbstractLiteral::Variant(field)) => &field.name,
                Expr::Atomic(
                    _,
                    Atom::Literal(Lit::AbstractLiteral(AbstractLiteral::Variant(field))),
                ) => &field.name,
                _ => return Err(RuleNotApplicable),
            };

            Ok(RuleEffect::pure(Expr::Atomic(
                m.clone(),
                Lit::Bool(active_alternative == alternative).into(),
            )))
        }
        // Semantic lowering for these lives in registered rules (function/set/mset/relation
        // horizontal rules), not here; the partial evaluator only offers extra constant-folding,
        // which `eval.rs`'s matching arms cover for the fully-constant case. Previously these
        // were `todo!()`, which panicked unconditionally on every rewrite of the operator (not
        // just constant ones), since `normalise_evaluator_local` calls this on every expression
        // node — the same landmine already found and fixed for Subsequence/Substring.
        Expr::Defined(_, _) => Err(RuleNotApplicable),
        Expr::Range(_, _) => Err(RuleNotApplicable),
        Expr::Image(_, _, _) => Err(RuleNotApplicable),
        Expr::ImageSet(_, _, _) => Err(RuleNotApplicable),
        Expr::PreImage(_, _, _) => Err(RuleNotApplicable),
        Expr::Inverse(_, _, _) => Err(RuleNotApplicable),
        Expr::PermInverse(_, _) => Err(RuleNotApplicable),
        Expr::Compose(_, _, _) => Err(RuleNotApplicable),
        Expr::Restrict(_, _, _) => Err(RuleNotApplicable),
        Expr::ToSet(_, _) => Err(RuleNotApplicable),
        Expr::ToMSet(_, _) => Err(RuleNotApplicable),
        Expr::ToRelation(_, _) => Err(RuleNotApplicable),
        Expr::RelationProj(_, _, _) => todo!(),
        Expr::Apart(_, _, _) => Err(RuleNotApplicable),
        Expr::Together(_, _, _) => Err(RuleNotApplicable),
        Expr::Participants(_, _) => Err(RuleNotApplicable),
        Expr::Party(_, _, _) => Err(RuleNotApplicable),
        Expr::Parts(_, _) => Err(RuleNotApplicable),
        Expr::Subsequence(_, _, _) => Err(RuleNotApplicable),
        Expr::Substring(_, _, _) => Err(RuleNotApplicable),
        Expr::LexLt(_, _, _) => Err(RuleNotApplicable),
        Expr::LexLeq(_, _, _) => Err(RuleNotApplicable),
        Expr::LexGt(_, _, _) => Err(RuleNotApplicable),
        Expr::LexGeq(_, _, _) => Err(RuleNotApplicable),
        Expr::FlatLexLt(_, _, _) => Err(RuleNotApplicable),
        Expr::FlatLexLeq(_, _, _) => Err(RuleNotApplicable),
        Expr::AllDifferentExcept(_, _, _) | Expr::ElementId(_, _, _) => Err(RuleNotApplicable),
    }
}

/// Whether an `and`/`or` operand is a matrix literal holding no elements.
///
/// This deliberately ignores the index domain: `unwrap_list*` only recognise the normalised
/// `int(1..)`, and an empty `[]` parses to `[;int(1..0)]`. Matching the elements directly folds
/// `and([])` / `or([])` before `matrix_to_list` normalises the domain, and so before any
/// solver-family rule can fire on an expression that is already known to be constant.
fn is_empty_matrix_operand(operand: &Expr) -> bool {
    match operand {
        Expr::TypeAnnotation(_, inner, _) | Expr::DomainAnnotation(_, inner, _) => {
            is_empty_matrix_operand(inner)
        }
        Expr::AbstractLiteral(_, AbstractLiteral::Matrix(elems, _)) => elems.is_empty(),
        Expr::Atomic(_, Atom::Literal(Lit::AbstractLiteral(AbstractLiteral::Matrix(elems, _)))) => {
            elems.is_empty()
        }
        _ => false,
    }
}

/// Extracts `lhs ▷ k` where `lhs` is atomic and `k` is an integer literal.
fn as_constant_bound_comparison(expr: &Expr) -> Option<(Atom, ConstantBoundOp, i32)> {
    as_constant_bound_comparison_ref(expr).map(|(lhs, op, rhs)| (lhs.clone(), op, rhs))
}

/// Borrowing variant of [`as_constant_bound_comparison`], for the inspection pass.
fn as_constant_bound_comparison_ref(expr: &Expr) -> Option<(&Atom, ConstantBoundOp, i32)> {
    let (lhs, rhs, op) = match expr {
        Expr::Gt(_, lhs, rhs) => (lhs, rhs, ConstantBoundOp::Gt),
        Expr::Geq(_, lhs, rhs) => (lhs, rhs, ConstantBoundOp::Geq),
        Expr::Lt(_, lhs, rhs) => (lhs, rhs, ConstantBoundOp::Lt),
        Expr::Leq(_, lhs, rhs) => (lhs, rhs, ConstantBoundOp::Leq),
        _ => return None,
    };

    let Expr::Atomic(_, Atom::Literal(Lit::Int(rhs_value))) = rhs.as_ref() else {
        return None;
    };
    let Expr::Atomic(_, lhs_atom) = lhs.as_ref() else {
        return None;
    };
    Some((lhs_atom, op, *rhs_value))
}

/// Rebuilds a constant bound comparison from its dominated components.
fn make_constant_bound_comparison(lhs: Atom, op: ConstantBoundOp, rhs: i32) -> Expr {
    let lhs = Expr::Atomic(Metadata::new(), lhs);
    let rhs = Expr::Atomic(Metadata::new(), Atom::Literal(Lit::Int(rhs)));
    match op {
        ConstantBoundOp::Gt => Expr::Gt(Metadata::new(), Moo::new(lhs), Moo::new(rhs)),
        ConstantBoundOp::Geq => Expr::Geq(Metadata::new(), Moo::new(lhs), Moo::new(rhs)),
        ConstantBoundOp::Lt => Expr::Lt(Metadata::new(), Moo::new(lhs), Moo::new(rhs)),
        ConstantBoundOp::Leq => Expr::Leq(Metadata::new(), Moo::new(lhs), Moo::new(rhs)),
    }
}

/// Keeps the strongest RHS for `(lhs, op)` under conjunction, preserving first-seen order.
///
/// Bounds are keyed rather than searched, so merging `n` bounds over distinct atoms -- the shape
/// unrolling `forAll i : D. x[i] >= k` produces -- is linear in `n` rather than quadratic.
/// `IndexMap` gives the lookup while keeping first-seen order.
fn merge_constant_bound(
    bounds: &mut IndexMap<(Atom, ConstantBoundOp), i32>,
    lhs: Atom,
    op: ConstantBoundOp,
    rhs: i32,
) {
    match bounds.entry((lhs, op)) {
        indexmap::map::Entry::Occupied(mut entry) => {
            let existing = entry.get_mut();
            if op.prefers_larger_rhs() {
                if rhs > *existing {
                    *existing = rhs;
                }
            } else if rhs < *existing {
                *existing = rhs;
            }
        }
        indexmap::map::Entry::Vacant(entry) => {
            entry.insert(rhs);
        }
    }
}

/// Checks for tautologies involving pairs of terms inside an or, returning true if one is found.
///
/// This applies the following rules:
///
/// ```text
/// (p->q) \/ (q->p) ~> true    [totality of implication]
/// (p->q) \/ (p-> !q) ~> true  [conditional excluded middle]
/// ```
///
fn check_pairwise_or_tautologies(or_terms: &[Expr]) -> bool {
    // `identical_atom_to` can only succeed when both sides are atomic, so index the atomic pairs
    // and look each candidate up. This is expected O(n), not the O(n^2) of comparing every pair.
    #[allow(clippy::mutable_key_type)]
    let mut p_implies_q: HashSet<(&Atom, &Atom)> = HashSet::new();
    #[allow(clippy::mutable_key_type)]
    let mut p_implies_not_q: HashSet<(&Atom, &Atom)> = HashSet::new();

    for term in or_terms {
        if let Expr::Imply(_, p, q) = term {
            if let Expr::Not(_, q_1) = q.as_ref() {
                if let (Expr::Atomic(_, p), Expr::Atomic(_, q)) = (p.as_ref(), q_1.as_ref()) {
                    p_implies_not_q.insert((p, q));
                }
            } else if let (Expr::Atomic(_, p), Expr::Atomic(_, q)) = (p.as_ref(), q.as_ref()) {
                p_implies_q.insert((p, q));
            }
        }
    }

    // `(p->q) \/ (q->p) ~> true    [totality of implication]`
    for &(p, q) in &p_implies_q {
        if p_implies_q.contains(&(q, p)) {
            return true;
        }
    }

    // `(p->q) \/ (p-> !q) ~> true`    [conditional excluded middle]
    p_implies_not_q
        .iter()
        .any(|pair| p_implies_q.contains(pair))
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::ast::{DeclarationPtr, Domain, Name};

    fn int_lit(value: i32) -> Expr {
        Expr::Atomic(Metadata::new(), Atom::Literal(Lit::Int(value)))
    }

    fn bool_lit(value: bool) -> Expr {
        Expr::Atomic(Metadata::new(), Atom::Literal(Lit::Bool(value)))
    }

    fn atom_ref(name: &str) -> Expr {
        Expr::Atomic(
            Metadata::new(),
            Atom::Reference(crate::ast::Reference::new(DeclarationPtr::new_find(
                Name::user(name),
                Domain::int(vec![Range::Bounded(1, 20)]),
            ))),
        )
    }

    fn singleton_ref(name: &str, value: i32) -> Expr {
        Expr::Atomic(
            Metadata::new(),
            Atom::Reference(crate::ast::Reference::new(DeclarationPtr::new_find(
                Name::user(name),
                Domain::int(vec![Range::Bounded(value, value)]),
            ))),
        )
    }

    fn safe_pow(base: Expr, exponent: Expr) -> Expr {
        Expr::SafePow(Metadata::new(), Moo::new(base), Moo::new(exponent))
    }

    /// A power folds when both operands are singletons, taking the base from a declaration domain.
    #[test]
    fn safe_pow_folds_singleton_operands() {
        let reduced = run_partial_evaluator_local(&safe_pow(singleton_ref("x", 2), int_lit(3)))
            .expect("evaluates")
            .new_expression;
        assert_eq!(reduced, int_lit(8));
    }

    /// A non-singleton operand leaves the power alone: its value is not yet known.
    #[test]
    fn safe_pow_keeps_non_singleton_operands() {
        assert!(run_partial_evaluator_local(&safe_pow(atom_ref("x"), int_lit(3))).is_err());
    }

    /// `0 ** 0` is undefined, and a negative exponent has no integer result.
    #[test]
    fn safe_pow_leaves_undefined_powers_alone() {
        assert!(run_partial_evaluator_local(&safe_pow(int_lit(0), int_lit(0))).is_err());
        assert!(run_partial_evaluator_local(&safe_pow(int_lit(2), int_lit(-1))).is_err());
    }

    /// Overflow is not folded: the model keeps the power rather than wrapping.
    #[test]
    fn safe_pow_leaves_overflowing_powers_alone() {
        assert!(run_partial_evaluator_local(&safe_pow(int_lit(i32::MAX), int_lit(2))).is_err());
    }

    fn and(exprs: Vec<Expr>) -> Expr {
        Expr::And(Metadata::new(), Moo::new(into_matrix_expr![exprs]))
    }

    fn or(exprs: Vec<Expr>) -> Expr {
        Expr::Or(Metadata::new(), Moo::new(into_matrix_expr![exprs]))
    }

    /// An empty conjunction is the identity of `and`.
    ///
    /// The partial evaluator is the only thing that implements this: its hook runs after every
    /// rewrite, so it reaches an empty `and`/`or` before any rule can.
    #[test]
    fn empty_and_is_true() {
        let reduced = run_partial_evaluator_local(&and(vec![]))
            .expect("evaluates")
            .new_expression;
        assert_eq!(reduced, bool_lit(true));
    }

    /// An empty disjunction is the identity of `or`. See [`empty_and_is_true`].
    #[test]
    fn empty_or_is_false() {
        let reduced = run_partial_evaluator_local(&or(vec![]))
            .expect("evaluates")
            .new_expression;
        assert_eq!(reduced, bool_lit(false));
    }

    /// An empty matrix written as `[]` parses with the *empty* index domain `int(1..0)`, not the
    /// normalised `int(1..)` that `unwrap_list` looks for. The evaluator must fold it regardless,
    /// so that no solver-family rule sees an expression that is already known to be constant.
    fn empty_matrix_with_empty_index_domain() -> Expr {
        Expr::AbstractLiteral(
            Metadata::new(),
            AbstractLiteral::Matrix(
                vec![],
                DomainPtr::from(Domain::int(vec![Range::Bounded(1, 0)])),
            ),
        )
    }

    #[test]
    fn empty_and_with_empty_index_domain_is_true() {
        let expr = Expr::And(
            Metadata::new(),
            Moo::new(empty_matrix_with_empty_index_domain()),
        );
        let reduced = run_partial_evaluator_local(&expr)
            .expect("evaluates")
            .new_expression;
        assert_eq!(reduced, bool_lit(true));
    }

    #[test]
    fn empty_or_with_empty_index_domain_is_false() {
        let expr = Expr::Or(
            Metadata::new(),
            Moo::new(empty_matrix_with_empty_index_domain()),
        );
        let reduced = run_partial_evaluator_local(&expr)
            .expect("evaluates")
            .new_expression;
        assert_eq!(reduced, bool_lit(false));
    }

    #[test]
    fn non_empty_and_is_not_collapsed_to_a_boolean() {
        let x = atom_ref("x");
        let expr = and(vec![Expr::Gt(
            Metadata::new(),
            Moo::new(x),
            Moo::new(int_lit(3)),
        )]);
        // Either it does not apply, or it rewrites to something that is not a bare boolean.
        if let Ok(reduced) = run_partial_evaluator_local(&expr) {
            assert_ne!(reduced.new_expression, bool_lit(true));
            assert_ne!(reduced.new_expression, bool_lit(false));
        }
    }

    #[test]
    fn and_dominates_constant_lower_bounds_on_same_atom() {
        let x = atom_ref("x");
        let expr = and(vec![
            Expr::Gt(Metadata::new(), Moo::new(x.clone()), Moo::new(int_lit(3))),
            bool_lit(true),
            Expr::Gt(Metadata::new(), Moo::new(x.clone()), Moo::new(int_lit(9))),
            Expr::Gt(Metadata::new(), Moo::new(x), Moo::new(int_lit(1))),
        ]);

        let reduced = run_partial_evaluator_local(&expr).unwrap().new_expression;
        let Expr::And(_, operands) = reduced else {
            panic!("expected And, got {reduced}");
        };
        let list = Moo::unwrap_or_clone(operands).unwrap_list().unwrap();
        assert_eq!(list.len(), 1);
        assert!(matches!(
            &list[0],
            Expr::Gt(_, lhs, rhs)
                if matches!(rhs.as_ref(), Expr::Atomic(_, Atom::Literal(Lit::Int(9))))
                    && matches!(lhs.as_ref(), Expr::Atomic(_, Atom::Reference(_)))
        ));
    }

    #[test]
    fn and_dominates_constant_upper_bounds_on_same_atom() {
        let x = atom_ref("x");
        let expr = and(vec![
            Expr::Leq(Metadata::new(), Moo::new(x.clone()), Moo::new(int_lit(8))),
            Expr::Leq(Metadata::new(), Moo::new(x), Moo::new(int_lit(4))),
        ]);

        let reduced = run_partial_evaluator_local(&expr).unwrap().new_expression;
        let Expr::And(_, operands) = reduced else {
            panic!("expected And, got {reduced}");
        };
        let list = Moo::unwrap_or_clone(operands).unwrap_list().unwrap();
        assert_eq!(list.len(), 1);
        assert!(matches!(
            &list[0],
            Expr::Leq(_, _, rhs)
                if matches!(rhs.as_ref(), Expr::Atomic(_, Atom::Literal(Lit::Int(4))))
        ));
    }

    #[test]
    fn and_does_not_merge_a_single_constant_bound() {
        let x = atom_ref("x");
        let expr = and(vec![Expr::Gt(
            Metadata::new(),
            Moo::new(x),
            Moo::new(int_lit(3)),
        )]);
        assert!(run_partial_evaluator_local(&expr).is_err());
    }

    fn product(factors: Vec<Expr>) -> Expr {
        Expr::Product(Metadata::new(), Moo::new(into_matrix_expr![factors]))
    }

    /// Local evaluation must not move a lone zero factor to the end of a product.
    ///
    /// That reshuffle undoes `reorder_product`'s constant-first form and causes an infinite
    /// rewrite loop on models such as `savilerow/diet` (`x[i] * 0`).
    #[test]
    fn local_product_does_not_reshuffle_zero_factor() {
        let x = atom_ref("x");
        let constant_first = product(vec![int_lit(0), x.clone()]);
        let variable_first = product(vec![x, int_lit(0)]);
        assert!(run_partial_evaluator_local(&constant_first).is_err());
        assert!(run_partial_evaluator_local(&variable_first).is_err());
    }

    /// Deep evaluation still collapses a safe `0 * x` product to the literal zero.
    #[test]
    fn deep_product_collapses_safe_zero_factor() {
        let expr = product(vec![int_lit(0), atom_ref("x")]);
        let reduced = run_partial_evaluator(&expr).unwrap().new_expression;
        assert_eq!(reduced, int_lit(0));
    }

    #[test]
    fn symbolic_cardinality_is_left_for_representation_rules() {
        let set = Expr::Atomic(
            Metadata::new(),
            Atom::Reference(crate::ast::Reference::new(DeclarationPtr::new_find(
                Name::user("s"),
                Domain::set(
                    crate::ast::SetAttr::new_max_size(2),
                    Domain::int(vec![Range::Bounded(1, 2)]),
                ),
            ))),
        );
        let cardinality = Expr::Card(Metadata::new(), Moo::new(set));

        assert!(run_partial_evaluator_local(&cardinality).is_err());
    }
}
