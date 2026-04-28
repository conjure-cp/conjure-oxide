use std::collections::HashSet;

use crate::ast::Typeable;
use crate::{
    ast::{
        AbstractLiteral, Atom, DomainPtr, Expression as Expr, GroundDomain, Literal as Lit,
        Metadata, Moo, Range, ReturnType,
    },
    into_matrix_expr,
    rule_engine::{ApplicationError::RuleNotApplicable, ApplicationResult, Reduction},
};
use itertools::iproduct;
use uniplate::Uniplate;

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
        if !expr.is_safe() {
            return false;
        }

        for subexpr in expr.universe() {
            let Expr::Atomic(_, Atom::Reference(reference)) = subexpr else {
                continue;
            };

            let Some(resolved) = reference.resolve_expression() else {
                continue;
            };

            let id = reference.id();
            if !resolving.insert(id.clone()) {
                return false;
            }

            let is_safe = helper(&resolved, resolving);
            resolving.remove(&id);

            if !is_safe {
                return false;
            }
        }

        true
    }

    helper(expr, &mut HashSet::new())
}

/// Tries to decide `expr in domain` from resolved domains alone.
fn simplify_in_domain(expr: &Expr, domain: &DomainPtr) -> Option<bool> {
    if !is_semantically_safe(expr) {
        return None;
    }

    let expr_domain = resolved_ground_domain_of_for_partial_eval(expr)?;
    let domain = domain.resolve()?;
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

/// Resolves a matrix literal subject, including constant references to matrix literals.
fn resolve_matrix_subject(subject: &Expr) -> Option<(Vec<Expr>, DomainPtr)> {
    subject.clone().unwrap_matrix_unchecked().or_else(|| {
        let Expr::Atomic(_, Atom::Reference(reference)) = subject else {
            return None;
        };

        let Lit::AbstractLiteral(AbstractLiteral::Matrix(elems, index_domain)) =
            reference.resolve_constant()?
        else {
            return None;
        };

        Some((
            elems
                .into_iter()
                .map(|elem| Expr::Atomic(Metadata::new(), Atom::Literal(elem)))
                .collect(),
            index_domain.into(),
        ))
    })
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
        _ => expr.domain_of()?.resolve(),
    }
}

/// Tries to decide `expr = lit` and `expr != lit` from the resolved domain of `expr`.
fn simplify_comparison_with_literal(expr: &Expr, lit: &Lit) -> Option<(bool, bool)> {
    if !is_semantically_safe(expr) {
        return None;
    }

    let expr_domain = resolved_ground_domain_of_for_partial_eval(expr)?;

    if !expr_domain.contains(lit).ok()? {
        return Some((false, true));
    }

    match (expr_domain.as_ref(), lit) {
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
        (GroundDomain::Bool, Lit::Bool(_)) => None,
        _ => None,
    }
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

pub fn run_partial_evaluator(expr: &Expr) -> ApplicationResult {
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
        Expr::FromSolution(_, _) => Err(RuleNotApplicable),
        Expr::Metavar(_, _) => Err(RuleNotApplicable),
        Expr::UnsafeIndex(_, _, _) => Err(RuleNotApplicable),
        Expr::UnsafeSlice(_, _, _) => Err(RuleNotApplicable),
        Expr::Table(_, _, _) => Err(RuleNotApplicable),
        Expr::NegativeTable(_, _, _) => Err(RuleNotApplicable),
        Expr::SafeIndex(_, subject, indices) => {
            // partially evaluate matrix literals indexed by a constant.

            // subject must be a matrix literal
            let (es, index_domain) = resolve_matrix_subject(subject).ok_or(RuleNotApplicable)?;

            if indices.is_empty() {
                return Err(RuleNotApplicable);
            }

            // the leading index must be fixed to a single value
            let index = singleton_int_value(&indices[0]).ok_or(RuleNotApplicable)?;

            // index domain must be a single integer range with a lower bound
            if let Some(ranges) = index_domain.as_int_ground()
                && ranges.len() == 1
                && let Some(from) = ranges[0].low()
            {
                let zero_indexed_index = index - from;
                let selected = es
                    .get(zero_indexed_index as usize)
                    .ok_or(RuleNotApplicable)?
                    .clone();

                if indices.len() == 1 {
                    Ok(Reduction::pure(selected))
                } else {
                    Ok(Reduction::pure(Expr::SafeIndex(
                        Metadata::new(),
                        Moo::new(selected),
                        indices[1..].to_vec(),
                    )))
                }
            } else {
                Err(RuleNotApplicable)
            }
        }
        Expr::SafeSlice(_, _, _) => Err(RuleNotApplicable),
        Expr::InDomain(_, x, domain) => {
            if let Some(result) = simplify_in_domain(x, domain) {
                Ok(Reduction::pure(Expr::Atomic(
                    Metadata::new(),
                    result.into(),
                )))
            } else if let Expr::Atomic(_, Atom::Reference(decl)) = x.as_ref() {
                let decl_domain = decl
                    .domain()
                    .ok_or(RuleNotApplicable)?
                    .resolve()
                    .ok_or(RuleNotApplicable)?;
                let domain = domain.resolve().ok_or(RuleNotApplicable)?;

                let intersection = decl_domain
                    .intersect(&domain)
                    .map_err(|_| RuleNotApplicable)?;

                // if the declaration's domain is a subset of domain, expr is always true.
                if &intersection == decl_domain.as_ref() {
                    Ok(Reduction::pure(Expr::Atomic(Metadata::new(), true.into())))
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
                    Ok(Reduction::pure(Expr::Atomic(Metadata::new(), false.into())))
                } else {
                    Err(RuleNotApplicable)
                }
            } else if let Expr::Atomic(_, Atom::Literal(lit)) = x.as_ref() {
                if domain
                    .resolve()
                    .ok_or(RuleNotApplicable)?
                    .contains(lit)
                    .ok()
                    .ok_or(RuleNotApplicable)?
                {
                    Ok(Reduction::pure(Expr::Atomic(Metadata::new(), true.into())))
                } else {
                    Ok(Reduction::pure(Expr::Atomic(Metadata::new(), false.into())))
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
                Ok(Reduction::pure(Moo::unwrap_or_clone(expr.clone())))
            } else {
                Err(RuleNotApplicable)
            }
        }
        Expr::Atomic(_, _) => Err(RuleNotApplicable),
        Expr::ToInt(_, expression) => {
            if expression.return_type() == ReturnType::Int {
                Ok(Reduction::pure(Moo::unwrap_or_clone(expression.clone())))
            } else {
                Err(RuleNotApplicable)
            }
        }
        Expr::Abs(m, e) => match e.as_ref() {
            Expr::Neg(_, inner) => Ok(Reduction::pure(Expr::Abs(m.clone(), inner.clone()))),
            _ => Err(RuleNotApplicable),
        },
        Expr::Sum(m, vec) => {
            let vec = Moo::unwrap_or_clone(vec.clone())
                .unwrap_list()
                .ok_or(RuleNotApplicable)?;
            let mut acc = 0;
            let mut n_consts = 0;
            let mut new_vec: Vec<Expr> = Vec::new();
            for expr in vec {
                if let Expr::Atomic(_, Atom::Literal(Lit::Int(x))) = expr {
                    acc += x;
                    n_consts += 1;
                } else {
                    new_vec.push(expr);
                }
            }
            if acc != 0 {
                new_vec.push(Expr::Atomic(
                    Default::default(),
                    Atom::Literal(Lit::Int(acc)),
                ));
            }

            if n_consts <= 1 {
                Err(RuleNotApplicable)
            } else {
                Ok(Reduction::pure(Expr::Sum(
                    m.clone(),
                    Moo::new(into_matrix_expr![new_vec]),
                )))
            }
        }

        Expr::Product(m, vec) => {
            let mut acc = 1;
            let mut n_consts = 0;
            let mut new_vec: Vec<Expr> = Vec::new();
            let vec = Moo::unwrap_or_clone(vec.clone())
                .unwrap_list()
                .ok_or(RuleNotApplicable)?;
            for expr in vec {
                if let Expr::Atomic(_, Atom::Literal(Lit::Int(x))) = expr {
                    acc *= x;
                    n_consts += 1;
                } else {
                    new_vec.push(expr);
                }
            }

            if n_consts == 0 {
                return Err(RuleNotApplicable);
            }

            new_vec.push(Expr::Atomic(
                Default::default(),
                Atom::Literal(Lit::Int(acc)),
            ));
            let new_product = Expr::Product(m.clone(), Moo::new(into_matrix_expr![new_vec]));

            if acc == 0 {
                // if safe, 0 * exprs ~> 0
                // otherwise, just return 0* exprs
                if is_semantically_safe(&new_product) {
                    Ok(Reduction::pure(Expr::Atomic(
                        Default::default(),
                        Atom::Literal(Lit::Int(0)),
                    )))
                } else {
                    Ok(Reduction::pure(new_product))
                }
            } else if n_consts == 1 {
                // acc !=0, only one constant
                Err(RuleNotApplicable)
            } else {
                // acc !=0, multiple constants found
                Ok(Reduction::pure(new_product))
            }
        }

        Expr::Min(m, e) => {
            let Some(vec) = Moo::unwrap_or_clone(e.clone()).unwrap_list() else {
                return Err(RuleNotApplicable);
            };
            let mut acc: Option<i32> = None;
            let mut n_consts = 0;
            let mut new_vec: Vec<Expr> = Vec::new();
            for expr in vec {
                if let Expr::Atomic(_, Atom::Literal(Lit::Int(x))) = expr {
                    n_consts += 1;
                    acc = match acc {
                        Some(i) => {
                            if i > x {
                                Some(x)
                            } else {
                                Some(i)
                            }
                        }
                        None => Some(x),
                    };
                } else {
                    new_vec.push(expr);
                }
            }

            if let Some(i) = acc {
                new_vec.push(Expr::Atomic(Default::default(), Atom::Literal(Lit::Int(i))));
            }

            if n_consts <= 1 {
                Err(RuleNotApplicable)
            } else {
                Ok(Reduction::pure(Expr::Min(
                    m.clone(),
                    Moo::new(into_matrix_expr![new_vec]),
                )))
            }
        }

        Expr::Max(m, e) => {
            let Some(vec) = Moo::unwrap_or_clone(e.clone()).unwrap_list() else {
                return Err(RuleNotApplicable);
            };

            let mut acc: Option<i32> = None;
            let mut n_consts = 0;
            let mut new_vec: Vec<Expr> = Vec::new();
            for expr in vec {
                if let Expr::Atomic(_, Atom::Literal(Lit::Int(x))) = expr {
                    n_consts += 1;
                    acc = match acc {
                        Some(i) => {
                            if i < x {
                                Some(x)
                            } else {
                                Some(i)
                            }
                        }
                        None => Some(x),
                    };
                } else {
                    new_vec.push(expr);
                }
            }

            if let Some(i) = acc {
                new_vec.push(Expr::Atomic(Default::default(), Atom::Literal(Lit::Int(i))));
            }

            if n_consts <= 1 {
                Err(RuleNotApplicable)
            } else {
                Ok(Reduction::pure(Expr::Max(
                    m.clone(),
                    Moo::new(into_matrix_expr![new_vec]),
                )))
            }
        }
        Expr::Not(_, e1) => {
            let Expr::Imply(_, p, q) = e1.as_ref() else {
                return Err(RuleNotApplicable);
            };

            if !is_semantically_safe(e1) {
                return Err(RuleNotApplicable);
            }

            match (p.as_ref(), q.as_ref()) {
                (_, Expr::Atomic(_, Atom::Literal(Lit::Bool(true)))) => {
                    Ok(Reduction::pure(Expr::from(false)))
                }
                (_, Expr::Atomic(_, Atom::Literal(Lit::Bool(false)))) => {
                    Ok(Reduction::pure(Moo::unwrap_or_clone(p.clone())))
                }
                (Expr::Atomic(_, Atom::Literal(Lit::Bool(true))), _) => {
                    Ok(Reduction::pure(Expr::Not(Metadata::new(), q.clone())))
                }
                (Expr::Atomic(_, Atom::Literal(Lit::Bool(false))), _) => {
                    Ok(Reduction::pure(Expr::from(false)))
                }
                _ => Err(RuleNotApplicable),
            }
        }
        Expr::Or(m, e) => {
            let Some(terms) = Moo::unwrap_or_clone(e.clone()).unwrap_list() else {
                return Err(RuleNotApplicable);
            };

            let mut has_changed = false;

            // 2. boolean literals
            let mut new_terms = vec![];
            for expr in terms {
                if let Expr::Atomic(_, Atom::Literal(Lit::Bool(x))) = expr {
                    has_changed = true;

                    // true ~~> entire or is true
                    // false ~~> remove false from the or
                    if x {
                        return Ok(Reduction::pure(true.into()));
                    }
                } else {
                    new_terms.push(expr);
                }
            }

            // 2. check pairwise tautologies.
            if check_pairwise_or_tautologies(&new_terms) {
                return Ok(Reduction::pure(true.into()));
            }

            // 3. empty or ~~> false
            if new_terms.is_empty() {
                return Ok(Reduction::pure(false.into()));
            }

            if !has_changed {
                return Err(RuleNotApplicable);
            }

            Ok(Reduction::pure(Expr::Or(
                m.clone(),
                Moo::new(into_matrix_expr![new_terms]),
            )))
        }
        Expr::And(_, e) => {
            let Some(vec) = Moo::unwrap_or_clone(e.clone()).unwrap_list() else {
                return Err(RuleNotApplicable);
            };
            let mut new_vec: Vec<Expr> = Vec::new();
            let mut has_const: bool = false;
            for expr in vec {
                if let Expr::Atomic(_, Atom::Literal(Lit::Bool(x))) = expr {
                    has_const = true;
                    if !x {
                        return Ok(Reduction::pure(Expr::Atomic(
                            Default::default(),
                            Atom::Literal(Lit::Bool(false)),
                        )));
                    }
                } else {
                    new_vec.push(expr);
                }
            }

            if !has_const {
                Err(RuleNotApplicable)
            } else {
                Ok(Reduction::pure(Expr::And(
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
                            return Ok(Reduction::pure(Expr::Root(
                                Metadata::new(),
                                vec![Expr::Atomic(
                                    Default::default(),
                                    Atom::Literal(Lit::Bool(false)),
                                )],
                            )));
                        }
                        // remove trues
                    }

                    // flatten ands in root
                    Expr::And(_, vecs) => match Moo::unwrap_or_clone(vecs.clone()).unwrap_list() {
                        Some(mut list) => {
                            has_changed = true;
                            new_vec.append(&mut list);
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
                Ok(Reduction::pure(Expr::Root(Metadata::new(), new_vec)))
            }
        }
        Expr::Imply(_m, x, y) => {
            if let Expr::Atomic(_, Atom::Literal(Lit::Bool(x))) = x.as_ref() {
                if *x {
                    // (true) -> y ~~> y
                    return Ok(Reduction::pure(Moo::unwrap_or_clone(y.clone())));
                } else {
                    // (false) -> y ~~> true
                    return Ok(Reduction::pure(Expr::Atomic(Metadata::new(), true.into())));
                }
            };

            if let Expr::Atomic(_, Atom::Literal(Lit::Bool(y))) = y.as_ref() {
                if *y {
                    // x -> (true) ~~> true
                    return Ok(Reduction::pure(Expr::from(true)));
                } else {
                    // x -> (false) ~~> !x
                    return Ok(Reduction::pure(Expr::Not(Metadata::new(), x.clone())));
                }
            };

            // reflexivity: p -> p ~> true

            // instead of checking syntactic equivalence of a possibly deep expression,
            // let identical-CSE turn them into identical variables first. Then, check if they are
            // identical variables.

            if x.identical_atom_to(y.as_ref()) && is_semantically_safe(x) && is_semantically_safe(y)
            {
                return Ok(Reduction::pure(true.into()));
            }

            Err(RuleNotApplicable)
        }
        Expr::Iff(_m, x, y) => {
            if let Expr::Atomic(_, Atom::Literal(Lit::Bool(x))) = x.as_ref() {
                if *x {
                    // (true) <-> y ~~> y
                    return Ok(Reduction::pure(Moo::unwrap_or_clone(y.clone())));
                } else {
                    // (false) <-> y ~~> !y
                    return Ok(Reduction::pure(Expr::Not(Metadata::new(), y.clone())));
                }
            };
            if let Expr::Atomic(_, Atom::Literal(Lit::Bool(y))) = y.as_ref() {
                if *y {
                    // x <-> (true) ~~> x
                    return Ok(Reduction::pure(Moo::unwrap_or_clone(x.clone())));
                } else {
                    // x <-> (false) ~~> !x
                    return Ok(Reduction::pure(Expr::Not(Metadata::new(), x.clone())));
                }
            };

            // reflexivity: p <-> p ~> true

            // instead of checking syntactic equivalence of a possibly deep expression,
            // let identical-CSE turn them into identical variables first. Then, check if they are
            // identical variables.

            if x.identical_atom_to(y.as_ref()) && is_semantically_safe(x) && is_semantically_safe(y)
            {
                return Ok(Reduction::pure(true.into()));
            }

            Err(RuleNotApplicable)
        }
        Expr::Eq(_, x, y) => {
            if let Some((eq_result, _)) = simplify_reflexive_comparison(x, y) {
                Ok(Reduction::pure(Expr::Atomic(
                    Metadata::new(),
                    Atom::Literal(Lit::Bool(eq_result)),
                )))
            } else if let Expr::Atomic(_, Atom::Literal(lit)) = x.as_ref()
                && let Some((eq_result, _)) = simplify_comparison_with_literal(y, lit)
            {
                Ok(Reduction::pure(Expr::Atomic(
                    Metadata::new(),
                    Atom::Literal(Lit::Bool(eq_result)),
                )))
            } else if let Expr::Atomic(_, Atom::Literal(lit)) = y.as_ref()
                && let Some((eq_result, _)) = simplify_comparison_with_literal(x, lit)
            {
                Ok(Reduction::pure(Expr::Atomic(
                    Metadata::new(),
                    Atom::Literal(Lit::Bool(eq_result)),
                )))
            } else {
                Err(RuleNotApplicable)
            }
        }
        Expr::Neq(_, x, y) => {
            if let Some((_, neq_result)) = simplify_reflexive_comparison(x, y) {
                Ok(Reduction::pure(Expr::Atomic(
                    Metadata::new(),
                    Atom::Literal(Lit::Bool(neq_result)),
                )))
            } else if let Expr::Atomic(_, Atom::Literal(lit)) = x.as_ref()
                && let Some((_, neq_result)) = simplify_comparison_with_literal(y, lit)
            {
                Ok(Reduction::pure(Expr::Atomic(
                    Metadata::new(),
                    Atom::Literal(Lit::Bool(neq_result)),
                )))
            } else if let Expr::Atomic(_, Atom::Literal(lit)) = y.as_ref()
                && let Some((_, neq_result)) = simplify_comparison_with_literal(x, lit)
            {
                Ok(Reduction::pure(Expr::Atomic(
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
            let Some(vec) = Moo::unwrap_or_clone(e.clone()).unwrap_list() else {
                return Err(RuleNotApplicable);
            };

            let mut consts: HashSet<i32> = HashSet::new();

            // check for duplicate constant values which would fail the constraint
            for expr in vec {
                if let Expr::Atomic(_, Atom::Literal(Lit::Int(x))) = expr
                    && !consts.insert(x)
                {
                    return Ok(Reduction::pure(Expr::Atomic(
                        m.clone(),
                        Atom::Literal(Lit::Bool(false)),
                    )));
                }
            }

            // nothing has changed
            Err(RuleNotApplicable)
        }
        Expr::Neg(_, _) => Err(RuleNotApplicable),
        Expr::Factorial(_, _) => Err(RuleNotApplicable),
        Expr::AuxDeclaration(_, _, _) => Err(RuleNotApplicable),
        Expr::UnsafeMod(_, _, _) => Err(RuleNotApplicable),
        Expr::SafeMod(_, _, _) => Err(RuleNotApplicable),
        Expr::UnsafePow(_, _, _) => Err(RuleNotApplicable),
        Expr::SafePow(_, _, _) => Err(RuleNotApplicable),
        Expr::Minus(_, _, _) => Err(RuleNotApplicable),

        // As these are in a low level solver form, I'm assuming that these have already been
        // simplified and partially evaluated.
        Expr::FlatAllDiff(_, _) => Err(RuleNotApplicable),
        Expr::FlatAbsEq(_, _, _) => Err(RuleNotApplicable),
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
        Expr::Defined(_, _) => todo!(),
        Expr::Range(_, _) => todo!(),
        Expr::Image(_, _, _) => todo!(),
        Expr::ImageSet(_, _, _) => todo!(),
        Expr::PreImage(_, _, _) => todo!(),
        Expr::Inverse(_, _, _) => todo!(),
        Expr::Restrict(_, _, _) => todo!(),
        Expr::ToSet(_, _) => todo!(),
        Expr::ToMSet(_, _) => todo!(),
        Expr::ToRelation(_, _) => todo!(),
        Expr::RelationProj(_, _, _) => todo!(),
        Expr::Apart(_, _, _) => todo!(),
        Expr::Together(_, _, _) => todo!(),
        Expr::Participants(_, _) => todo!(),
        Expr::Party(_, _, _) => todo!(),
        Expr::Parts(_, _) => todo!(),
        Expr::Subsequence(_, _, _) => todo!(),
        Expr::Substring(_, _, _) => todo!(),
        Expr::LexLt(_, _, _) => Err(RuleNotApplicable),
        Expr::LexLeq(_, _, _) => Err(RuleNotApplicable),
        Expr::LexGt(_, _, _) => Err(RuleNotApplicable),
        Expr::LexGeq(_, _, _) => Err(RuleNotApplicable),
        Expr::FlatLexLt(_, _, _) => Err(RuleNotApplicable),
        Expr::FlatLexLeq(_, _, _) => Err(RuleNotApplicable),
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
    // Collect terms that are structurally identical to the rule input.
    // Then, try the rules on these terms, also checking the other conditions of the rules.

    // stores (p,q) in p -> q
    let mut p_implies_q: Vec<(&Expr, &Expr)> = vec![];

    // stores (p,q) in p -> !q
    let mut p_implies_not_q: Vec<(&Expr, &Expr)> = vec![];

    for term in or_terms.iter() {
        if let Expr::Imply(_, p, q) = term {
            // we use identical_atom_to for equality later on, so these sets are mutually exclusive.
            //
            // in general however, p -> !q would be in p_implies_q as (p,!q)
            if let Expr::Not(_, q_1) = q.as_ref() {
                p_implies_not_q.push((p.as_ref(), q_1.as_ref()));
            } else {
                p_implies_q.push((p.as_ref(), q.as_ref()));
            }
        }
    }

    // `(p->q) \/ (q->p) ~> true    [totality of implication]`
    for ((p1, q1), (q2, p2)) in iproduct!(p_implies_q.iter(), p_implies_q.iter()) {
        if p1.identical_atom_to(p2) && q1.identical_atom_to(q2) {
            return true;
        }
    }

    // `(p->q) \/ (p-> !q) ~> true`    [conditional excluded middle]
    for ((p1, q1), (p2, q2)) in iproduct!(p_implies_q.iter(), p_implies_not_q.iter()) {
        if p1.identical_atom_to(p2) && q1.identical_atom_to(q2) {
            return true;
        }
    }

    false
}
