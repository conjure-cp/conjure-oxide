//! Value-level collection bounds; domain unions remain common type envelopes.

use super::{
    Domain, DomainPtr, Expression, GroundDomain, IntVal, Range, ReturnType, SetAttr, Typeable,
};

/// Cardinalities use wider arithmetic so an overflowing upper bound can be omitted safely.
#[derive(Clone, Copy, Debug)]
pub(super) struct SizeBounds {
    pub low: i64,
    pub high: Option<i64>,
}

impl SizeBounds {
    pub fn new(low: i64, high: Option<i64>) -> Self {
        Self {
            low: low.max(0),
            high,
        }
    }

    /// Resolve endpoints independently: a symbolic endpoint must not hide a known one.
    pub fn from_size(size: &Range<IntVal>) -> Self {
        let (low, high) = match size {
            Range::Single(n) => (Some(n), Some(n)),
            Range::Bounded(low, high) => (Some(low), Some(high)),
            Range::UnboundedL(high) => (None, Some(high)),
            Range::UnboundedR(low) => (Some(low), None),
            Range::Unbounded => (None, None),
        };
        Self::new(
            low.and_then(|x| x.resolve().ok())
                .map(i64::from)
                .unwrap_or(0),
            high.and_then(|x| x.resolve().ok()).map(i64::from),
        )
    }

    pub fn cap(mut self, capacity: Option<i64>) -> Self {
        self.high = min_known(self.high, capacity);
        self
    }

    pub fn range(self) -> Option<Range<i32>> {
        if self.high.is_some_and(|high| high < self.low) {
            return None;
        }
        Some(Range::new(
            Some(self.low.min(i64::from(i32::MAX)) as i32),
            self.high.and_then(|high| i32::try_from(high).ok()),
        ))
    }
}

pub(super) fn min_known(a: Option<i64>, b: Option<i64>) -> Option<i64> {
    match (a, b) {
        (Some(a), Some(b)) => Some(a.min(b)),
        (a, b) => a.or(b),
    }
}

/// Count integer values using interval arithmetic, including ranges wider than i32.
pub(super) fn capacity(domain: &DomainPtr) -> Option<i64> {
    ground_capacity(domain.resolve().ok()?.as_ref())
}

fn ground_capacity(ground: &GroundDomain) -> Option<i64> {
    if let GroundDomain::Int(ranges) = ground {
        let mut intervals = ranges
            .iter()
            .map(|range| Some((i64::from(*range.low()?), i64::from(*range.high()?))))
            .collect::<Option<Vec<_>>>()?;
        intervals.sort_unstable();
        let mut total = 0i64;
        let mut previous: Option<(i64, i64)> = None;
        for (low, high) in intervals {
            if low > high {
                continue;
            }
            match previous {
                Some((start, end)) if low <= end + 1 => previous = Some((start, end.max(high))),
                Some((start, end)) => {
                    total = total.checked_add(end - start + 1)?;
                    previous = Some((low, high));
                }
                None => previous = Some((low, high)),
            }
        }
        if let Some((start, end)) = previous {
            total = total.checked_add(end - start + 1)?;
        }
        Some(total)
    } else {
        match ground {
            GroundDomain::Empty(_) => Some(0),
            GroundDomain::Bool => Some(2),
            GroundDomain::Tuple(domains) => domains
                .iter()
                .try_fold(1i64, |n, d| n.checked_mul(ground_capacity(d)?)),
            GroundDomain::Record(fields) => fields
                .iter()
                .try_fold(1i64, |n, f| n.checked_mul(ground_capacity(&f.value)?)),
            GroundDomain::Variant(fields) => fields
                .iter()
                .try_fold(0i64, |n, f| n.checked_add(ground_capacity(&f.value)?)),
            // Avoid potentially expensive combinatorial counting for nested collections.
            _ => None,
        }
    }
}

/// Return an overlap envelope without enumerating integer values.
pub(super) fn overlap_domain(a: &DomainPtr, b: &DomainPtr) -> DomainPtr {
    if a == b {
        return a.clone();
    }
    if let (Ok(a_ground), Ok(b_ground)) = (a.resolve(), b.resolve()) {
        match (a_ground.as_ref(), b_ground.as_ref()) {
            (GroundDomain::Empty(_), _) => return DomainPtr::from(a_ground),
            (_, GroundDomain::Empty(_)) => return DomainPtr::from(b_ground),
            (GroundDomain::Int(a_ranges), GroundDomain::Int(b_ranges)) => {
                let ranges = a_ranges
                    .iter()
                    .flat_map(|a| {
                        b_ranges
                            .iter()
                            .filter_map(move |b| Range::minimal(&[a.clone(), b.clone()]).ok())
                    })
                    .collect();
                return Domain::int(ranges);
            }
            _ => {}
        }
    }
    // The left domain always contains the overlap, even for unsupported/symbolic types.
    a.clone()
}

pub(super) fn bounded_set(bounds: SizeBounds, inner: DomainPtr) -> DomainPtr {
    match bounds.cap(capacity(&inner)).range() {
        Some(size) => Domain::set(SetAttr::new(size), inner),
        None => Domain::empty(ReturnType::Set(Box::new(inner.return_type()))),
    }
}

fn represented_set(
    bounds: SizeBounds,
    inner: DomainPtr,
    representation: Option<String>,
) -> DomainPtr {
    let mut result = bounded_set(bounds, inner);
    if let Some(representation) = representation {
        if let Some((attrs, _)) = result.as_set_ground_mut() {
            attrs.representation = Some(representation);
        } else if let Some((attrs, _)) = result.as_set_mut() {
            attrs.representation = Some(representation);
        }
    }
    result
}

#[derive(Clone, Copy)]
pub(super) enum SetOperation {
    Union,
    Intersect,
    Difference,
}

pub(super) fn set_operation_domain(
    a: &Expression,
    b: &Expression,
    op: SetOperation,
) -> Option<DomainPtr> {
    let (a_attr, a_inner) = a.domain_of()?.as_set()?;
    let (b_attr, b_inner) = b.domain_of()?.as_set()?;
    let a_bounds = SizeBounds::from_size(&a_attr.size).cap(capacity(&a_inner));
    let b_bounds = SizeBounds::from_size(&b_attr.size).cap(capacity(&b_inner));
    if a_bounds.range().is_none() || b_bounds.range().is_none() {
        return Some(Domain::empty(ReturnType::Set(Box::new(
            a_inner.return_type(),
        ))));
    }
    let a_rep = super::expressions::selected_representation(a).or(a_attr.representation);
    let b_rep = super::expressions::selected_representation(b).or(b_attr.representation);
    let representation = match op {
        SetOperation::Difference => a_rep,
        _ => match (a_rep, b_rep) {
            (Some(a), Some(b)) if a == b => Some(a),
            (Some(a), None) | (None, Some(a)) => Some(a),
            _ => None,
        },
    };
    if a_bounds.high == Some(0) || b_bounds.high == Some(0) {
        let (bounds, inner) = match op {
            SetOperation::Union if a_bounds.high == Some(0) => (b_bounds, b_inner),
            SetOperation::Union | SetOperation::Difference => (a_bounds, a_inner),
            SetOperation::Intersect => (
                SizeBounds::new(0, Some(0)),
                if a_bounds.high == Some(0) {
                    b_inner
                } else {
                    a_inner
                },
            ),
        };
        return Some(represented_set(bounds, inner, representation));
    }
    if a == b {
        return Some(match op {
            SetOperation::Union | SetOperation::Intersect => {
                represented_set(a_bounds, a_inner, representation)
            }
            SetOperation::Difference => {
                represented_set(SizeBounds::new(0, Some(0)), a_inner, representation)
            }
        });
    }
    let overlap = overlap_domain(&a_inner, &b_inner);
    let overlap_capacity = capacity(&overlap);
    let (bounds, inner) = match op {
        SetOperation::Union => {
            let low = overlap_capacity
                .map(|n| (a_bounds.low + b_bounds.low - n).max(0))
                .unwrap_or(0)
                .max(a_bounds.low)
                .max(b_bounds.low);
            let high = a_bounds
                .high
                .zip(b_bounds.high)
                .and_then(|(a, b)| a.checked_add(b));
            (SizeBounds::new(low, high), a_inner.union(&b_inner).ok()?)
        }
        SetOperation::Intersect => {
            let universe = a_inner.union(&b_inner).ok()?;
            let low = capacity(&universe)
                .map(|n| (a_bounds.low + b_bounds.low - n).max(0))
                .unwrap_or(0);
            let high = min_known(min_known(a_bounds.high, b_bounds.high), overlap_capacity);
            (SizeBounds::new(low, high), overlap)
        }
        SetOperation::Difference => {
            let removable = min_known(b_bounds.high, overlap_capacity);
            let low = removable.map(|n| (a_bounds.low - n).max(0)).unwrap_or(0);
            (SizeBounds::new(low, a_bounds.high), a_inner)
        }
    };
    Some(represented_set(bounds, inner, representation))
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::ast::{AbstractLiteral, DeclarationPtr, Literal, Metadata, Moo, Name, Reference};
    use std::collections::BTreeSet;

    fn variable(name: &str, domain: DomainPtr) -> Expression {
        Reference::new(DeclarationPtr::new_find(Name::user(name), domain)).into()
    }

    fn operation(a: &Expression, b: &Expression, op: SetOperation) -> Expression {
        let a = Moo::new(a.clone());
        let b = Moo::new(b.clone());
        match op {
            SetOperation::Union => Expression::Union(Metadata::new(), a, b),
            SetOperation::Intersect => Expression::Intersect(Metadata::new(), a, b),
            SetOperation::Difference => Expression::Difference(Metadata::new(), a, b),
        }
    }

    #[test]
    fn set_operations_cover_every_small_operand_value() {
        let mut cases = Vec::new();
        for support in 0..8 {
            let inner = Domain::int(
                (0..3)
                    .filter(|i| support & (1 << i) != 0)
                    .map(Range::Single)
                    .collect(),
            );
            for low in 0..=3 {
                for high in low..=3 {
                    let domain = Domain::set(SetAttr::new_min_max_size(low, high), inner.clone());
                    let values = domain
                        .resolve()
                        .unwrap()
                        .values()
                        .unwrap()
                        .collect::<Vec<_>>();
                    if !values.is_empty() {
                        cases.push((domain, values));
                    }
                }
            }
        }
        for (a_domain, a_values) in &cases {
            for (b_domain, b_values) in &cases {
                let a = variable("a", a_domain.clone());
                let b = variable("b", b_domain.clone());
                for op in [
                    SetOperation::Union,
                    SetOperation::Intersect,
                    SetOperation::Difference,
                ] {
                    let result = operation(&a, &b, op)
                        .domain_of()
                        .unwrap()
                        .resolve()
                        .unwrap();
                    for a in a_values {
                        for b in b_values {
                            let Literal::AbstractLiteral(AbstractLiteral::Set(a)) = a else {
                                unreachable!()
                            };
                            let Literal::AbstractLiteral(AbstractLiteral::Set(b)) = b else {
                                unreachable!()
                            };
                            let a = a
                                .iter()
                                .map(|x| match x {
                                    Literal::Int(n) => *n,
                                    _ => unreachable!(),
                                })
                                .collect::<BTreeSet<_>>();
                            let b = b
                                .iter()
                                .map(|x| match x {
                                    Literal::Int(n) => *n,
                                    _ => unreachable!(),
                                })
                                .collect::<BTreeSet<_>>();
                            let elems = match op {
                                SetOperation::Union => {
                                    a.union(&b).copied().map(Literal::Int).collect()
                                }
                                SetOperation::Intersect => {
                                    a.intersection(&b).copied().map(Literal::Int).collect()
                                }
                                SetOperation::Difference => {
                                    a.difference(&b).copied().map(Literal::Int).collect()
                                }
                            };
                            let value = Literal::AbstractLiteral(AbstractLiteral::Set(elems));
                            assert!(
                                result.contains(&value).unwrap(),
                                "{a_domain} / {b_domain}: {value} outside {result}"
                            );
                        }
                    }
                }
            }
        }
    }

    #[test]
    fn set_bounds_use_capacity_overlap_and_identity() {
        let inner = Domain::int(vec![Range::Bounded(1, 3)]);
        let a = variable("a", Domain::set(SetAttr::new_size(2), inner.clone()));
        let b = variable("b", Domain::set(SetAttr::new_size(2), inner));
        let intersection = operation(&a, &b, SetOperation::Intersect)
            .domain_of()
            .unwrap();
        assert_eq!(
            intersection.as_set_ground().unwrap().0.size,
            Range::Bounded(1, 2)
        );
        let union = operation(&a, &b, SetOperation::Union).domain_of().unwrap();
        assert_eq!(union.as_set_ground().unwrap().0.size, Range::Bounded(2, 3));
        let difference = operation(&a, &b, SetOperation::Difference)
            .domain_of()
            .unwrap();
        assert_eq!(
            difference.as_set_ground().unwrap().0.size,
            Range::Bounded(0, 2)
        );
        let same = operation(&a, &a, SetOperation::Difference)
            .domain_of()
            .unwrap();
        assert_eq!(same.as_set_ground().unwrap().0.size, Range::Single(0));
    }

    #[test]
    fn large_set_bounds_do_not_overflow_or_enumerate_support() {
        let inner = Domain::int(vec![Range::Bounded(0, i32::MAX)]);
        let a = variable(
            "a",
            Domain::set(SetAttr::new_max_size(i32::MAX), inner.clone()),
        );
        let b = variable("b", Domain::set(SetAttr::new_max_size(i32::MAX), inner));
        let union = operation(&a, &b, SetOperation::Union).domain_of().unwrap();
        assert_eq!(union.as_set_ground().unwrap().0.size, Range::UnboundedR(0));
        let intersection = operation(&a, &b, SetOperation::Intersect)
            .domain_of()
            .unwrap();
        assert_eq!(
            intersection.as_set_ground().unwrap().0.size,
            Range::Bounded(0, i32::MAX)
        );
    }

    #[test]
    fn symbolic_endpoints_keep_known_bounds_and_representation() {
        let n = IntVal::Reference(Reference::new(DeclarationPtr::new_given(
            Name::user("n"),
            Domain::int(vec![Range::Bounded(1, 4)]),
        )));
        let a = variable(
            "a",
            Domain::set(
                SetAttr::new(Range::Bounded(IntVal::from(1), n)).with_representation("packed"),
                Domain::int(vec![Range::Bounded(1, 4)]),
            ),
        );
        let b = variable(
            "b",
            Domain::set(
                SetAttr::new_max_size(1),
                Domain::int(vec![Range::Single(1)]),
            ),
        );
        let result = operation(&a, &b, SetOperation::Union).domain_of().unwrap();
        let (attrs, _) = result.as_set_ground().unwrap();
        assert_eq!(attrs.size, Range::Bounded(1, 4));
        assert_eq!(attrs.representation.as_deref(), Some("packed"));
        let result = operation(&a, &b, SetOperation::Intersect)
            .domain_of()
            .unwrap();
        assert_eq!(result.as_set_ground().unwrap().0.size, Range::Bounded(0, 1));
    }

    #[test]
    fn impossible_operand_is_distinct_from_an_empty_set() {
        let inner = Domain::int(vec![Range::Single(1)]);
        let impossible = variable("a", Domain::set(SetAttr::new_size(2), inner.clone()));
        let empty = variable("b", Domain::set(SetAttr::new_size(0), inner));
        let result = operation(&impossible, &empty, SetOperation::Union)
            .domain_of()
            .unwrap();
        assert!(matches!(
            result.as_ground(),
            Some(GroundDomain::Empty(ReturnType::Set(_)))
        ));
        let result = operation(&empty, &empty, SetOperation::Union)
            .domain_of()
            .unwrap();
        assert_eq!(result.as_set_ground().unwrap().0.size, Range::Single(0));
    }
}
