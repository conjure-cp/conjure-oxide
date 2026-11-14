//! Domain-calculation helpers for expressions.
//!
//! Expression dispatch and its metadata cache live in `expressions`; arithmetic,
//! matrix and collection bounds are calculated here without expanding integer domains.

use std::collections::BTreeSet;

use super::collection_domains::{SizeBounds, min_known, mset_size};
use super::comprehension::ComprehensionQualifier;
use super::eval::factorial_i32;
use super::{
    Atom, Domain, DomainPtr, Expression, GroundDomain, HasDomain, MSetAttr, Moo, Range, ReturnType,
    Typeable, UnresolvedDomain,
};

// for the given matrix literal, return a bounded domain from the min to max of applying op to each
// child expression.
//
// Op must be monotonic.
//
// Returns none if unbounded
pub(super) fn bounded_i32_domain_for_matrix_literal_monotonic(
    e: &Expression,
    op: fn(i32, i32) -> Option<i32>,
) -> Option<DomainPtr> {
    // only care about the elements, not the indices
    let (mut exprs, _) = e.clone().unwrap_matrix_unchecked()?;
    //
    // here, I assume that op is monotone. This means that the bounds of op([a1,a2],[b1,b2])  for
    // the ranges [a1,a2], [b1,b2] will be
    // [min(op(a1,b1),op(a2,b1),op(a1,b2),op(a2,b2)),max(op(a1,b1),op(a2,b1),op(a1,b2),op(a2,b2))].
    //
    // We used to not assume this, and work out the bounds by applying op on the Cartesian product
    // of A and B; however, this caused a combinatorial explosion and my computer to run out of
    // memory (on the hakank_eprime_xkcd test)...
    //Int
    // For example, to find the bounds of the intervals [1,4], [1,5] combined using op, we used to do
    //  [min(op(1,1), op(1,2),op(1,3),op(1,4),op(1,5),op(2,1)..
    //
    // +,-,/,* are all monotone, so this assumption should be fine for now...

    let expr = exprs.pop()?;
    let dom = expr.domain_of()?;
    let resolved = dom.resolve().ok()?;
    let GroundDomain::Int(ranges, _) = resolved.as_ref() else {
        return None;
    };

    let (mut current_min, mut current_max) = range_vec_bounds_i32(ranges)?;

    for expr in exprs {
        let dom = expr.domain_of()?;
        let resolved = dom.resolve().ok()?;
        let GroundDomain::Int(ranges, _) = resolved.as_ref() else {
            return None;
        };

        let (min, max) = range_vec_bounds_i32(ranges)?;

        // all the possible new values for current_min / current_max
        let minmax = op(min, current_max)?;
        let minmin = op(min, current_min)?;
        let maxmin = op(max, current_min)?;
        let maxmax = op(max, current_max)?;
        let vals = [minmax, minmin, maxmin, maxmax];

        current_min = *vals
            .iter()
            .min()
            .expect("vals iterator should not be empty, and should have a minimum.");
        current_max = *vals
            .iter()
            .max()
            .expect("vals iterator should not be empty, and should have a maximum.");
    }

    if current_min == current_max {
        Some(Domain::int(vec![Range::Single(current_min)]))
    } else {
        Some(Domain::int(vec![Range::Bounded(current_min, current_max)]))
    }
}

pub(super) fn matrix_element_domain(e: &Expression) -> Option<DomainPtr> {
    let (elem_domain, _) = e.domain_of()?.as_matrix()?;
    elem_domain.as_ref().as_int()?;
    Some(elem_domain)
}

pub(super) fn empty_matrix_integer_element_domain(e: &Expression) -> Option<DomainPtr> {
    match e {
        Expression::TypeAnnotation(_, inner, domain) => {
            if !Moo::unwrap_or_clone(inner.clone())
                .unwrap_matrix_unchecked()
                .is_some_and(|(elems, _)| elems.is_empty())
            {
                return None;
            }
            if let ReturnType::Matrix(elem_type) = domain.return_type()
                && elem_type.as_ref() == &ReturnType::Int
            {
                Some(Domain::int_ground(vec![Range::Unbounded]))
            } else {
                None
            }
        }
        Expression::DomainAnnotation(_, inner, domain) => {
            if !Moo::unwrap_or_clone(inner.clone())
                .unwrap_matrix_unchecked()
                .is_some_and(|(elems, _)| elems.is_empty())
            {
                return None;
            }
            let (elem_domain, _) = domain.as_matrix()?;
            elem_domain.as_ref().as_int()?;
            Some(elem_domain)
        }
        _ => {
            if !e
                .clone()
                .unwrap_matrix_unchecked()
                .is_some_and(|(elems, _)| elems.is_empty())
            {
                return None;
            }
            matrix_element_domain(e)
        }
    }
}

// Returns none if unbounded
fn range_vec_bounds_i32(ranges: &Vec<Range<i32>>) -> Option<(i32, i32)> {
    let mut min = i32::MAX;
    let mut max = i32::MIN;
    for r in ranges {
        match r {
            Range::Single(i) => {
                if *i < min {
                    min = *i;
                }
                if *i > max {
                    max = *i;
                }
            }
            Range::Bounded(i, j) => {
                if *i < min {
                    min = *i;
                }
                if *j > max {
                    max = *j;
                }
            }
            Range::UnboundedR(_) | Range::UnboundedL(_) | Range::Unbounded => return None,
        }
    }
    Some((min, max))
}

/// Integer ranges without expanding their values. Typed empty domains are
/// accepted so arithmetic over an empty operand remains empty.
fn finite_integer_ranges(expr: &Expression) -> Option<Vec<Range<i32>>> {
    let domain = expr.domain_of()?.resolve().ok()?;
    match domain.as_ref() {
        GroundDomain::Int(ranges, _) if ranges.iter().all(Range::is_finite) => Some(ranges.clone()),
        GroundDomain::Empty(ReturnType::Int) => Some(Vec::new()),
        _ => None,
    }
}

/// Bounds for operations whose extrema occur at the corners of two intervals.
/// Overflow means we cannot supply a sound i32 domain, rather than silently
/// omitting values or wrapping an endpoint.
pub(super) fn binary_integer_domain(
    a: &Expression,
    b: &Expression,
    op: fn(i32, i32) -> Option<i32>,
) -> Option<DomainPtr> {
    let a = finite_integer_ranges(a)?;
    let b = finite_integer_ranges(b)?;
    if a.is_empty() || b.is_empty() {
        return Some(Domain::empty(ReturnType::Int));
    }
    let (a_min, a_max) = range_vec_bounds_i32(&a)?;
    let (b_min, b_max) = range_vec_bounds_i32(&b)?;
    let corners = [
        op(a_min, b_min)?,
        op(a_min, b_max)?,
        op(a_max, b_min)?,
        op(a_max, b_max)?,
    ];
    Some(Domain::int(vec![Range::new(
        corners.iter().min().copied(),
        corners.iter().max().copied(),
    )]))
}

/// Absolute value maps each input interval to one interval. Retain holes without
/// enumerating the integers, and decline inference if abs(MIN) would overflow.
pub(super) fn absolute_integer_domain(a: &Expression) -> Option<DomainPtr> {
    let mut result = Vec::new();
    for range in finite_integer_ranges(a)? {
        let (lo, hi) = (*range.low()?, *range.high()?);
        let (a, b) = (lo.checked_abs()?, hi.checked_abs()?);
        let lower = if lo <= 0 && hi >= 0 { 0 } else { a.min(b) };
        result.push(Range::new(Some(lower), Some(a.max(b))));
    }
    if result.is_empty() {
        return Some(Domain::empty(ReturnType::Int));
    }
    result.sort_unstable_by_key(|range| range.low().copied());
    let mut merged: Vec<Range<i32>> = Vec::new();
    for range in result {
        if let Some(last) = merged.last_mut()
            && let Some(joined) = last.join(&range)
        {
            *last = joined;
        } else {
            merged.push(range);
        }
    }
    Some(Domain::int(merged))
}

/// Floor division is monotonic on each side of zero. Only numerator endpoints
/// and the endpoints of each nonzero divisor interval can be extrema. Widen the
/// quotient calculation so MIN / -1 does not panic; out-of-i32 results are
/// excluded, as in floor_div, without excluding representable neighbouring values.
pub(super) fn division_domain(a: &Expression, b: &Expression, safe: bool) -> Option<DomainPtr> {
    let a = finite_integer_ranges(a)?;
    let b = finite_integer_ranges(b)?;
    let mut result = Vec::new();
    if !a.is_empty() && !b.is_empty() {
        let (a_min, a_max) = range_vec_bounds_i32(&a)?;
        let mut low = i64::MAX;
        let mut high = i64::MIN;
        for range in b {
            let (lo, hi) = (*range.low()?, *range.high()?);
            // Splitting at zero is essential: checking only the outer endpoints
            // of -N..N would miss the extreme quotients at -1 and 1.
            let endpoints = [lo, hi.min(-1), lo.max(1), hi];
            for divisor in endpoints
                .into_iter()
                .filter(|b| *b != 0 && lo <= *b && *b <= hi)
            {
                for dividend in [a_min, a_max] {
                    let remainder = super::floor_mod(dividend, divisor)?;
                    let quotient =
                        (i64::from(dividend) - i64::from(remainder)) / i64::from(divisor);
                    low = low.min(quotient);
                    high = high.max(quotient);
                }
            }
        }
        let low = low.max(i64::from(i32::MIN));
        let high = high.min(i64::from(i32::MAX));
        if low <= high {
            result.push(Range::new(
                Some(i32::try_from(low).ok()?),
                Some(i32::try_from(high).ok()?),
            ));
        }
    }
    if safe {
        result.push(Range::Single(0));
    }
    if result.is_empty() {
        Some(Domain::empty(ReturnType::Int))
    } else {
        Some(Domain::int(Range::squeeze(&result)))
    }
}

/// Power extrema occur at base interval endpoints or -1/0/1, and at the first
/// and last exponent of each parity. This is independent of the number of base
/// values or exponents; checked_pow also takes only logarithmic time in the
/// exponent. Keep the existing defined-result semantics (negative exponents and
/// 0**0 contribute no values), and decline inference on possible i32 overflow.
pub(super) fn power_integer_domain(a: &Expression, b: &Expression) -> Option<DomainPtr> {
    let a = finite_integer_ranges(a)?;
    let b = finite_integer_ranges(b)?;
    let mut low = i32::MAX;
    let mut high = i32::MIN;
    for base in a {
        let (lo, hi) = (*base.low()?, *base.high()?);
        for exponent in &b {
            let (first, last) = (
                i64::from((*exponent.low()?).max(0)),
                i64::from(*exponent.high()?),
            );
            for parity in [0, 1] {
                let first = first + i64::from(first % 2 != parity);
                let last = last - i64::from(last.rem_euclid(2) != parity);
                if first > last {
                    continue;
                }
                for power in [first, last] {
                    let power = u32::try_from(power).ok()?;
                    for value in [lo, hi, -1, 0, 1]
                        .into_iter()
                        .filter(|value| lo <= *value && *value <= hi)
                    {
                        if value == 0 && power == 0 {
                            continue;
                        }
                        let result = value.checked_pow(power)?;
                        low = low.min(result);
                        high = high.max(result);
                    }
                }
            }
        }
    }
    if low > high {
        Some(Domain::empty(ReturnType::Int))
    } else {
        Some(Domain::int(vec![Range::new(Some(low), Some(high))]))
    }
}

/// Only 0!..12! are representable as i32. Validate endpoints before allocating
/// any values, then inspect this fixed set to preserve the exact sparse result.
pub(super) fn factorial_domain(a: &Expression) -> Option<DomainPtr> {
    let ranges = finite_integer_ranges(a)?;
    if ranges.is_empty() {
        return Some(Domain::empty(ReturnType::Int));
    }
    let (lo, hi) = range_vec_bounds_i32(&ranges)?;
    if lo < 0 || hi > 12 {
        return None;
    }
    let values = (0..=12)
        .filter(|value| ranges.iter().any(|range| range.contains(value)))
        .map(factorial_i32)
        .collect::<Option<BTreeSet<_>>>()?;
    Some(DomainPtr::from(GroundDomain::from_set_i32(&values)))
}

/// Sound floor-remainder bounds, computed from range endpoints rather than the
/// Cartesian product of operand values. Inference is linear in the number of
/// ranges, independent of their cardinality. Holes in the result are permitted:
/// the modulo constraint, rather than its auxiliary domain, enforces exactness.
pub(super) fn modulo_domain(a: &Expression, b: &Expression, safe: bool) -> Option<DomainPtr> {
    let a_domain = a.domain_of()?.resolve().ok()?;
    let b_domain = b.domain_of()?.resolve().ok()?;
    let ranges = |domain: &GroundDomain| match domain {
        GroundDomain::Int(ranges, _) => Some(ranges.clone()),
        GroundDomain::Empty(ReturnType::Int) => Some(Vec::new()),
        _ => None,
    };
    let a_ranges = ranges(&a_domain)?;
    let b_ranges = ranges(&b_domain)?;
    let empty = || {
        if safe {
            Domain::int(vec![Range::Single(0)])
        } else {
            Domain::empty(ReturnType::Int)
        }
    };
    if a_ranges.is_empty() || b_ranges.is_empty() {
        return Some(empty());
    }
    let (a_min, a_max) = range_vec_bounds_i32(&a_ranges)?;
    let (b_min, b_max) = range_vec_bounds_i32(&b_ranges)?;
    if b_min == 0 && b_max == 0 {
        return Some(empty());
    }
    // Keep constant inference exact, including MIN % -1 (which is zero).
    if a_min == a_max && b_min == b_max {
        let value = super::floor_mod(a_min, b_min)?;
        let mut ranges = vec![Range::Single(value)];
        if safe && value != 0 {
            ranges.push(Range::Single(0));
        }
        return Some(Domain::int(ranges));
    }
    // For b > 0, 0 <= a mod b < b; for b < 0, b < a mod b <= 0.
    // Add/subtract only on the corresponding sign branch, avoiding overflow
    // even when an endpoint is i32::MIN or i32::MAX.
    let lo = if b_min < 0 { b_min + 1 } else { 0 };
    let hi = if b_max > 0 { b_max - 1 } else { 0 };
    // If dividend and divisor have the same sign, the magnitude of the
    // remainder cannot exceed the magnitude of the dividend either.
    let mut lo = if a_max <= 0 { lo.max(a_min) } else { lo };
    let mut hi = if a_min >= 0 { hi.min(a_max) } else { hi };
    if safe {
        lo = lo.min(0);
        hi = hi.max(0);
    }
    Some(Domain::int(vec![Range::new(Some(lo), Some(hi))]))
}

/// Integer domain bounds for [`Expression::Sum`] over a comprehension.
fn sum_domain_for_comprehension(expr: &Expression) -> Option<DomainPtr> {
    let Expression::Comprehension(_, comp) = expr else {
        return None;
    };
    let comp = comp.as_ref();
    let resolved = comp.return_expression.domain_of()?.resolve().ok()?;
    let GroundDomain::Int(return_ranges, _) = resolved.as_ref() else {
        return None;
    };
    let (term_min, term_max) = range_vec_bounds_i32(return_ranges)?;

    let mut term_count = 1i64;
    for qual in &comp.qualifiers {
        match qual {
            ComprehensionQualifier::Generator { ptr } => {
                let count = ptr.domain()?.resolve().ok()?.length().ok()?;
                term_count = term_count.saturating_mul(count as i64);
                if term_count > i32::MAX as i64 {
                    return None;
                }
            }
            ComprehensionQualifier::Condition(_)
            | ComprehensionQualifier::ExpressionGenerator { .. } => {
                return None;
            }
        }
    }

    if term_count == 0 {
        return Some(Domain::int(vec![Range::Single(0)]));
    }

    let n = term_count as i32;
    let sum_min = n.saturating_mul(term_min);
    let sum_max = n.saturating_mul(term_max);
    if sum_min == sum_max {
        Some(Domain::int(vec![Range::Single(sum_min)]))
    } else {
        Some(Domain::int(vec![Range::Bounded(sum_min, sum_max)]))
    }
}

pub(super) fn sum_domain_of_child(child: &Expression) -> Option<DomainPtr> {
    sum_domain_for_comprehension(child).or_else(|| {
        let (elements, _) = child.clone().unwrap_matrix_unchecked()?;
        if elements.len() == 1 {
            sum_domain_for_comprehension(&elements[0])
        } else {
            None
        }
    })
}

pub(super) fn selected_representation(expr: &Expression) -> Option<String> {
    match expr {
        Expression::Atomic(_, Atom::Reference(reference)) => reference
            .repr
            .map(|rule| rule.short_name().to_owned())
            .or_else(|| {
                reference
                    .domain_of()
                    .representation_preference()
                    .map(str::to_owned)
            }),
        Expression::TypeAnnotation(_, inner, domain)
        | Expression::DomainAnnotation(_, inner, domain) => domain
            .representation_preference()
            .map(str::to_owned)
            .or_else(|| selected_representation(inner)),
        _ => None,
    }
}

/// The value-level union of two multisets adds their cardinalities and occurrence counts.
///
/// [`Domain::union`] instead computes a common type envelope and intentionally drops collection
/// attributes, so it is too weak for the domain of an `a union b` expression.
pub(super) fn mset_union_result_domain(lhs: &Expression, rhs: &Expression) -> Option<DomainPtr> {
    let lhs_selected = selected_representation(lhs);
    let rhs_selected = selected_representation(rhs);
    let (lhs_attrs, lhs_inner) = lhs.domain_of()?.as_mset()?;
    let (rhs_attrs, rhs_inner) = rhs.domain_of()?.as_mset()?;
    let lhs_size = mset_size(&lhs_attrs, &lhs_inner);
    let rhs_size = mset_size(&rhs_attrs, &rhs_inner);
    if lhs_size.range().is_none() || rhs_size.range().is_none() {
        return Some(Domain::empty(ReturnType::MSet(Box::new(
            lhs_inner.return_type(),
        ))));
    }
    let inner = if lhs_size.high == Some(0) {
        rhs_inner
    } else if rhs_size.high == Some(0) {
        lhs_inner
    } else {
        lhs_inner.union(&rhs_inner).ok()?
    };
    let size = SizeBounds::new(
        lhs_size.low.checked_add(rhs_size.low)?,
        lhs_size
            .high
            .zip(rhs_size.high)
            .and_then(|(a, b)| a.checked_add(b)),
    )
    .range()?;
    let lhs_occurrence = min_known(
        SizeBounds::from_size(&lhs_attrs.occurrence).high,
        lhs_size.high,
    );
    let rhs_occurrence = min_known(
        SizeBounds::from_size(&rhs_attrs.occurrence).high,
        rhs_size.high,
    );
    let max_occurrence = lhs_occurrence
        .zip(rhs_occurrence)
        .and_then(|(a, b)| a.checked_add(b));
    let occurrence = SizeBounds::new(0, max_occurrence).range()?;
    // minOccur applies only to present values, including the empty multiset vacuously.
    let occurrence = if max_occurrence == Some(0) {
        occurrence
    } else {
        Range::new(Some(1), occurrence.high().copied())
    };
    let representation = match (
        lhs_selected
            .as_deref()
            .or(lhs_attrs.representation.as_deref()),
        rhs_selected
            .as_deref()
            .or(rhs_attrs.representation.as_deref()),
    ) {
        (Some(lhs), Some(rhs)) if lhs == rhs => Some(lhs.to_owned()),
        (Some(preference), None) | (None, Some(preference)) => Some(preference.to_owned()),
        _ => None,
    };

    let mut attrs = MSetAttr::new(size, occurrence);
    attrs.representation = representation;
    Some(Domain::mset(attrs, inner))
}

/// Also handles permutations: `image(perm, x)` returns a value from the permutation's own inner
/// domain (a permutation's "codomain" is its domain), so this doubles as the codomain lookup for
/// `Expression::Image`/`ImageSet` applied to either a function or a permutation.
pub(super) fn get_function_codomain(function: &Moo<Expression>) -> Option<DomainPtr> {
    let function_domain = function.domain_of()?;
    match function_domain.resolve().as_ref() {
        Ok(d) => {
            match d.as_ref() {
                GroundDomain::Function(_, _, codomain) => Some(codomain.clone().into()),
                GroundDomain::Permutation(_, inner) => Some(inner.clone().into()),
                GroundDomain::Sequence(_, inner) => Some(inner.clone().into()),
                // Not defined for anything other than a function, permutation or sequence
                _ => None,
            }
        }
        Err(_) => {
            match function_domain.as_unresolved()? {
                UnresolvedDomain::Function(_, _, codomain) => Some(codomain.clone()),
                UnresolvedDomain::Permutation(_, inner) => Some(inner.clone()),
                UnresolvedDomain::Sequence(_, inner) => Some(inner.clone()),
                // Not defined for anything other than a function, permutation or sequence
                _ => None,
            }
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::ast::{AbstractLiteral, DeclarationPtr, Literal, Metadata, Name, Reference};
    use crate::matrix_expr;

    fn integer_variable(domain: DomainPtr) -> Expression {
        Reference::new(DeclarationPtr::new_find(Name::user("x"), domain)).into()
    }

    #[test]
    fn interval_arithmetic_domains_cover_all_small_signed_results() {
        type Binary = fn(Metadata, Moo<Expression>, Moo<Expression>) -> Expression;
        type Operation = fn(i32, i32) -> Option<i32>;
        let operations: [(Binary, Operation); 3] = [
            (Expression::Minus, i32::checked_sub),
            (Expression::PairwiseSum, i32::checked_add),
            (Expression::PairwiseProduct, i32::checked_mul),
        ];
        for a_min in -3..=3 {
            for a_max in a_min..=3 {
                let a = integer_variable(Domain::int(vec![Range::Bounded(a_min, a_max)]));
                let abs = Expression::Abs(Metadata::new(), Moo::new(a.clone()));
                let absolute = abs.domain_of().unwrap().resolve().unwrap();
                for value in a_min..=a_max {
                    assert!(absolute.contains(&Literal::Int(value.abs())).unwrap());
                }
                for b_min in -3..=3 {
                    for b_max in b_min..=3 {
                        let b = integer_variable(Domain::int(vec![Range::Bounded(b_min, b_max)]));
                        for (construct, operation) in operations {
                            let expr = construct(
                                Metadata::new(),
                                Moo::new(a.clone()),
                                Moo::new(b.clone()),
                            );
                            let domain = expr.domain_of().unwrap().resolve().unwrap();
                            for x in a_min..=a_max {
                                for y in b_min..=b_max {
                                    let result = operation(x, y).unwrap();
                                    assert!(
                                        domain.contains(&Literal::Int(result)).unwrap(),
                                        "{expr}: {result} missing from {domain}"
                                    );
                                }
                            }
                        }
                    }
                }
            }
        }
    }

    #[test]
    fn interval_arithmetic_handles_large_domains_empty_operands_and_overflow() {
        let wide = integer_variable(Domain::int(vec![Range::Bounded(
            -1_000_000_000,
            1_000_000_000,
        )]));
        let two = integer_variable(Domain::int(vec![Range::Bounded(-2, 2)]));
        for expr in [
            Expression::Minus(
                Metadata::new(),
                Moo::new(wide.clone()),
                Moo::new(wide.clone()),
            ),
            Expression::PairwiseSum(
                Metadata::new(),
                Moo::new(wide.clone()),
                Moo::new(wide.clone()),
            ),
            Expression::PairwiseProduct(Metadata::new(), Moo::new(wide.clone()), Moo::new(two)),
        ] {
            assert_eq!(
                expr.domain_of(),
                Some(Domain::int(vec![Range::Bounded(
                    -2_000_000_000,
                    2_000_000_000
                )]))
            );
        }
        assert_eq!(
            Expression::Abs(Metadata::new(), Moo::new(wide.clone())).domain_of(),
            Some(Domain::int(vec![Range::Bounded(0, 1_000_000_000)]))
        );
        let sparse = integer_variable(Domain::int(vec![
            Range::Bounded(-8, -5),
            Range::Bounded(3, 4),
            Range::Single(0),
        ]));
        assert_eq!(
            Expression::Abs(Metadata::new(), Moo::new(sparse))
                .domain_of()
                .unwrap()
                .resolve()
                .unwrap()
                .values_i32()
                .unwrap(),
            vec![0, 3, 4, 5, 6, 7, 8]
        );
        let min = integer_variable(Domain::int(vec![Range::Single(i32::MIN)]));
        let max = integer_variable(Domain::int(vec![Range::Single(i32::MAX)]));
        let one = integer_variable(Domain::int(vec![Range::Single(1)]));
        assert_eq!(
            Expression::Abs(Metadata::new(), Moo::new(min.clone())).domain_of(),
            None
        );
        for expr in [
            Expression::Minus(Metadata::new(), Moo::new(min), Moo::new(one.clone())),
            Expression::PairwiseSum(Metadata::new(), Moo::new(max.clone()), Moo::new(one)),
            Expression::PairwiseProduct(Metadata::new(), Moo::new(max.clone()), Moo::new(max)),
        ] {
            assert_eq!(expr.domain_of(), None);
        }
        let empty = integer_variable(Domain::empty(ReturnType::Int));
        assert_eq!(
            Expression::Abs(Metadata::new(), Moo::new(empty.clone())).domain_of(),
            Some(Domain::empty(ReturnType::Int))
        );
        for construct in [
            Expression::Minus,
            Expression::PairwiseSum,
            Expression::PairwiseProduct,
        ] {
            for (a, b) in [(empty.clone(), wide.clone()), (wide.clone(), empty.clone())] {
                assert_eq!(
                    construct(Metadata::new(), Moo::new(a), Moo::new(b)).domain_of(),
                    Some(Domain::empty(ReturnType::Int))
                );
            }
        }
    }

    fn division_expression(a: DomainPtr, b: DomainPtr, safe: bool) -> Expression {
        let (a, b) = (Moo::new(integer_variable(a)), Moo::new(integer_variable(b)));
        if safe {
            Expression::SafeDiv(Metadata::new(), a, b)
        } else {
            Expression::UnsafeDiv(Metadata::new(), a, b)
        }
    }

    #[test]
    fn division_domain_bounds_cover_all_small_signed_results() {
        for a_min in -4..=4 {
            for a_max in a_min..=4 {
                for b_min in -4..=4 {
                    for b_max in b_min..=4 {
                        for safe in [false, true] {
                            let expr = division_expression(
                                Domain::int(vec![Range::Bounded(a_min, a_max)]),
                                Domain::int(vec![Range::Bounded(b_min, b_max)]),
                                safe,
                            );
                            let domain = expr.domain_of().unwrap().resolve().unwrap();
                            let expected: BTreeSet<_> = (a_min..=a_max)
                                .flat_map(|a| {
                                    (b_min..=b_max)
                                        .filter_map(move |b| super::super::floor_div(a, b))
                                })
                                .chain(safe.then_some(0))
                                .collect();
                            for value in &expected {
                                assert!(
                                    domain.contains(&Literal::Int(*value)).unwrap(),
                                    "{expr}: {value} missing from {domain}"
                                );
                            }
                            if expected.is_empty() || (a_min == a_max && b_min == b_max) {
                                assert_eq!(
                                    domain
                                        .values_i32()
                                        .unwrap()
                                        .into_iter()
                                        .collect::<BTreeSet<_>>(),
                                    expected
                                );
                            }
                        }
                    }
                }
            }
        }
    }

    #[test]
    fn division_domain_bounds_handle_large_sparse_empty_and_extreme_operands() {
        let wide = Domain::int(vec![Range::Bounded(-1_000_000_000, 1_000_000_000)]);
        let full = Domain::int(vec![Range::Bounded(i32::MIN, i32::MAX)]);
        for safe in [false, true] {
            assert_eq!(
                division_expression(wide.clone(), wide.clone(), safe).domain_of(),
                Some(wide.clone())
            );
            assert_eq!(
                division_expression(full.clone(), full.clone(), safe).domain_of(),
                Some(full.clone())
            );
            for empty in [
                Domain::empty(ReturnType::Int),
                Domain::int(Vec::<Range<i32>>::new()),
                Domain::int(vec![Range::Single(0)]),
            ] {
                let domain = division_expression(wide.clone(), empty, safe)
                    .domain_of()
                    .unwrap()
                    .resolve()
                    .unwrap();
                assert_eq!(
                    domain.values_i32().unwrap(),
                    if safe { vec![0] } else { vec![] }
                );
            }
            let min = Domain::int(vec![Range::Single(i32::MIN)]);
            let minus_one = Domain::int(vec![Range::Single(-1)]);
            let domain = division_expression(min, minus_one.clone(), safe)
                .domain_of()
                .unwrap()
                .resolve()
                .unwrap();
            assert_eq!(
                domain.values_i32().unwrap(),
                if safe { vec![0] } else { vec![] }
            );
            let near_min = Domain::int(vec![Range::Bounded(i32::MIN, i32::MIN + 1)]);
            let domain = division_expression(near_min, minus_one, safe)
                .domain_of()
                .unwrap()
                .resolve()
                .unwrap();
            assert!(domain.contains(&Literal::Int(i32::MAX)).unwrap());
            let sparse = Domain::int(vec![Range::Single(-9), Range::Bounded(3, 5)]);
            let domain = division_expression(sparse.clone(), sparse, safe)
                .domain_of()
                .unwrap()
                .resolve()
                .unwrap();
            for a in [-9, 3, 4, 5] {
                for b in [-9, 3, 4, 5] {
                    assert!(
                        domain
                            .contains(&Literal::Int(super::super::floor_div(a, b).unwrap()))
                            .unwrap()
                    );
                }
            }
        }
    }

    #[test]
    fn power_domain_bounds_cover_all_small_signed_results() {
        for a_min in -3..=3 {
            for a_max in a_min..=3 {
                for b_min in -2..=5 {
                    for b_max in b_min..=5 {
                        for construct in [Expression::SafePow, Expression::UnsafePow] {
                            let a =
                                integer_variable(Domain::int(vec![Range::Bounded(a_min, a_max)]));
                            let b =
                                integer_variable(Domain::int(vec![Range::Bounded(b_min, b_max)]));
                            let expr = construct(Metadata::new(), Moo::new(a), Moo::new(b));
                            let domain = expr.domain_of().unwrap().resolve().unwrap();
                            let expected: BTreeSet<_> = (a_min..=a_max)
                                .flat_map(|a: i32| {
                                    (b_min..=b_max).filter_map(move |b| {
                                        if b >= 0 && (a != 0 || b != 0) {
                                            a.checked_pow(b as u32)
                                        } else {
                                            None
                                        }
                                    })
                                })
                                .collect();
                            for result in &expected {
                                assert!(
                                    domain.contains(&Literal::Int(*result)).unwrap(),
                                    "{expr}: {result} missing from {domain}"
                                );
                            }
                            // Besides containment, the extrema must be tight.
                            let actual = domain.values_i32().unwrap();
                            assert_eq!(actual.iter().min(), expected.iter().next());
                            assert_eq!(actual.iter().max(), expected.iter().next_back());
                        }
                    }
                }
            }
        }
    }

    #[test]
    fn power_and_factorial_domains_do_not_materialise_large_operands() {
        for construct in [Expression::SafePow, Expression::UnsafePow] {
            let power = |base, exponent| {
                construct(
                    Metadata::new(),
                    Moo::new(integer_variable(base)),
                    Moo::new(integer_variable(exponent)),
                )
            };
            let wide = Domain::int(vec![Range::Bounded(-1_000_000_000, 1_000_000_000)]);
            let one = Domain::int(vec![Range::Single(1)]);
            assert_eq!(
                power(wide.clone(), one.clone()).domain_of(),
                Some(wide.clone())
            );
            let huge_exponent = Domain::int(vec![Range::Bounded(-1_000_000_000, i32::MAX)]);
            assert_eq!(
                power(Domain::int(vec![Range::Bounded(-1, 1)]), huge_exponent).domain_of(),
                Some(Domain::int(vec![Range::Bounded(-1, 1)]))
            );
            let thirty_one = Domain::int(vec![Range::Single(31)]);
            assert_eq!(
                power(Domain::int(vec![Range::Single(-2)]), thirty_one.clone()).domain_of(),
                Some(Domain::int(vec![Range::Single(i32::MIN)]))
            );
            assert_eq!(
                power(Domain::int(vec![Range::Single(2)]), thirty_one).domain_of(),
                None
            );
            assert_eq!(
                power(wide.clone(), Domain::int(vec![Range::Bounded(0, i32::MAX)])).domain_of(),
                None
            );
            assert_eq!(
                power(Domain::empty(ReturnType::Int), one).domain_of(),
                Some(Domain::empty(ReturnType::Int))
            );
        }
        let factorial =
            |domain| Expression::Factorial(Metadata::new(), Moo::new(integer_variable(domain)));
        for domain in [
            Domain::int(vec![Range::Bounded(0, i32::MAX)]),
            Domain::int(vec![Range::Bounded(i32::MIN, i32::MAX)]),
        ] {
            assert_eq!(factorial(domain).domain_of(), None);
        }
        for low in 0..=12 {
            for high in low..=12 {
                let expected = (low..=high)
                    .map(|n| factorial_i32(n).unwrap())
                    .collect::<BTreeSet<_>>();
                let result = factorial(Domain::int(vec![Range::Bounded(low, high)]))
                    .domain_of()
                    .unwrap()
                    .resolve()
                    .unwrap();
                assert_eq!(
                    result
                        .values_i32()
                        .unwrap()
                        .into_iter()
                        .collect::<BTreeSet<_>>(),
                    expected
                );
            }
        }
        let sparse = Domain::int(vec![Range::Single(0), Range::Single(4), Range::Single(12)]);
        let values = factorial(sparse)
            .domain_of()
            .unwrap()
            .resolve()
            .unwrap()
            .values_i32()
            .unwrap();
        assert_eq!(values, vec![1, 24, 479001600]);
    }

    fn modulo_expression(a: DomainPtr, b: DomainPtr, safe: bool) -> Expression {
        let variable = |name: &str, domain| {
            Moo::new(Expression::Atomic(
                Metadata::new(),
                Atom::Reference(Reference::new(DeclarationPtr::new_find(
                    Name::user(name),
                    domain,
                ))),
            ))
        };
        let (a, b) = (variable("a", a), variable("b", b));
        if safe {
            Expression::SafeMod(Metadata::new(), a, b)
        } else {
            Expression::UnsafeMod(Metadata::new(), a, b)
        }
    }

    #[test]
    fn modulo_domain_bounds_contain_all_small_signed_results() {
        // Exhaust all pairs of small intervals, including constants, mixed signs,
        // zero-only divisors, and same-sign dividend tightening.
        for a_min in -4..=4 {
            for a_max in a_min..=4 {
                for b_min in -4..=4 {
                    for b_max in b_min..=4 {
                        for safe in [false, true] {
                            let domain = modulo_expression(
                                Domain::int(vec![Range::Bounded(a_min, a_max)]),
                                Domain::int(vec![Range::Bounded(b_min, b_max)]),
                                safe,
                            )
                            .domain_of()
                            .unwrap()
                            .resolve()
                            .unwrap();
                            let expected: BTreeSet<_> = (a_min..=a_max)
                                .flat_map(|a| {
                                    (b_min..=b_max)
                                        .filter_map(move |b| super::super::floor_mod(a, b))
                                })
                                .chain(safe.then_some(0))
                                .collect();
                            for value in &expected {
                                assert!(
                                    domain.contains(&Literal::Int(*value)).unwrap(),
                                    "{a_min}..{a_max} mod {b_min}..{b_max}, safe={safe}: {value} missing from {domain}"
                                );
                            }
                            if expected.is_empty() {
                                assert_eq!(domain.len_usize().unwrap(), 0);
                            }
                            if a_min == a_max && b_min == b_max {
                                assert_eq!(
                                    domain
                                        .values_i32()
                                        .unwrap()
                                        .into_iter()
                                        .collect::<BTreeSet<_>>(),
                                    expected
                                );
                            }
                        }
                    }
                }
            }
        }
    }

    #[test]
    fn modulo_domain_bounds_handle_sparse_empty_and_unbounded_domains() {
        let sparse = Domain::int(vec![Range::Single(-7), Range::Bounded(3, 5)]);
        for safe in [false, true] {
            let domain = modulo_expression(sparse.clone(), sparse.clone(), safe)
                .domain_of()
                .unwrap()
                .resolve()
                .unwrap();
            for a in [-7, 3, 4, 5] {
                for b in [-7, 3, 4, 5] {
                    let value = super::super::floor_mod(a, b).unwrap();
                    assert!(domain.contains(&Literal::Int(value)).unwrap());
                }
            }
            for empty in [
                Domain::empty(ReturnType::Int),
                Domain::int(Vec::<Range<i32>>::new()),
            ] {
                for (a, b) in [(empty.clone(), sparse.clone()), (sparse.clone(), empty)] {
                    let domain = modulo_expression(a, b, safe)
                        .domain_of()
                        .unwrap()
                        .resolve()
                        .unwrap();
                    assert_eq!(
                        domain.values_i32().unwrap(),
                        if safe { vec![0] } else { vec![] }
                    );
                }
            }
            for (a, b) in [
                (Domain::int(vec![Range::<i32>::Unbounded]), sparse.clone()),
                (sparse.clone(), Domain::int(vec![Range::<i32>::Unbounded])),
                (Domain::bool(), sparse.clone()),
                (sparse.clone(), Domain::bool()),
            ] {
                assert_eq!(modulo_expression(a, b, safe).domain_of(), None);
            }
        }
    }

    #[test]
    fn modulo_domain_bounds_do_not_enumerate_large_domains() {
        // The previous Cartesian-product algorithm needed 400 million pairs for
        // +/-10,000, and could not even allocate the full i32 domain below.
        for bound in [100, 10_000, 1_000_000_000] {
            let domain = Domain::int(vec![Range::Bounded(-bound, bound)]);
            for safe in [false, true] {
                assert_eq!(
                    modulo_expression(domain.clone(), domain.clone(), safe).domain_of(),
                    Some(Domain::int(vec![Range::Bounded(-bound + 1, bound - 1)]))
                );
            }
        }
        let full = Domain::int(vec![Range::Bounded(i32::MIN, i32::MAX)]);
        for safe in [false, true] {
            assert_eq!(
                modulo_expression(full.clone(), full.clone(), safe).domain_of(),
                Some(Domain::int(vec![Range::Bounded(
                    i32::MIN + 1,
                    i32::MAX - 1
                )]))
            );
            let min = Domain::int(vec![Range::Single(i32::MIN)]);
            let minus_one = Domain::int(vec![Range::Single(-1)]);
            assert_eq!(
                modulo_expression(min, minus_one, safe).domain_of(),
                Some(Domain::int(vec![Range::Single(0)]))
            );
        }
    }

    #[test]
    fn test_domain_of_constant_sum() {
        let c1 = Expression::Atomic(Metadata::new(), Atom::Literal(Literal::Int(1)));
        let c2 = Expression::Atomic(Metadata::new(), Atom::Literal(Literal::Int(2)));
        let sum = Expression::Sum(Metadata::new(), Moo::new(matrix_expr![c1, c2]));
        assert_eq!(sum.domain_of(), Some(Domain::int(vec![Range::Single(3)])));
    }

    #[test]
    fn test_domain_of_constant_invalid_type() {
        let c1 = Expression::Atomic(Metadata::new(), Atom::Literal(Literal::Int(1)));
        let c2 = Expression::Atomic(Metadata::new(), Atom::Literal(Literal::Bool(true)));
        let sum = Expression::Sum(Metadata::new(), Moo::new(matrix_expr![c1, c2]));
        assert_eq!(sum.domain_of(), None);
    }

    #[test]
    fn test_domain_of_empty_sum() {
        let sum = Expression::Sum(Metadata::new(), Moo::new(matrix_expr![]));
        assert_eq!(sum.domain_of(), None);
    }

    /// Product domain inference must not panic when bound corner-products overflow i32.
    #[test]
    fn test_domain_of_product_overflow_returns_none() {
        let mk_var = |name: &str| {
            Expression::Atomic(
                Metadata::new(),
                Atom::Reference(Reference::new(DeclarationPtr::new_find(
                    Name::User(name.into()),
                    Domain::int(vec![Range::Bounded(0, 711)]),
                ))),
            )
        };
        let product = Expression::Product(
            Metadata::new(),
            Moo::new(matrix_expr![
                mk_var("item1"),
                mk_var("item2"),
                mk_var("item3"),
                mk_var("item4")
            ]),
        );
        assert_eq!(product.domain_of(), None);
    }

    #[test]
    fn mset_union_domain_adds_finite_operand_bounds() {
        let lhs = DeclarationPtr::new_find(
            Name::user("xs"),
            Domain::mset(
                MSetAttr::new_max_size(6).with_representation("counts"),
                Domain::int(vec![Range::Bounded(1, 999)]),
            ),
        );
        let rhs = Expression::AbstractLiteral(
            Metadata::new(),
            AbstractLiteral::MSet(vec![1.into(), 2.into()]),
        );
        let union = Expression::Union(
            Metadata::new(),
            Moo::new(Expression::from(Reference::new(lhs))),
            Moo::new(rhs),
        );

        let domain = union.domain_of().expect("multiset union has a domain");
        let (attrs, inner) = domain.as_mset_ground().expect("ground multiset domain");
        assert_eq!(attrs.size, Range::Bounded(2, 8));
        assert_eq!(attrs.occurrence, Range::Bounded(1, 8));
        assert_eq!(attrs.representation.as_deref(), Some("counts"));
        assert_eq!(
            inner.as_ref(),
            &GroundDomain::Int(vec![Range::Bounded(1, 999)], None)
        );
    }
}
