//! Value-level collection bounds; domain unions remain common type envelopes.

use super::{
    Domain, DomainPtr, Expression, GroundDomain, IntVal, JectivityAttr, MSetAttr, Range,
    ReturnType, SetAttr, Typeable,
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

pub(super) fn product_capacity(domains: &[DomainPtr]) -> Option<i64> {
    // A known empty factor makes the product zero even with unresolved other factors.
    let lengths = domains.iter().map(capacity).collect::<Vec<_>>();
    if lengths.contains(&Some(0)) {
        return Some(0);
    }
    lengths
        .into_iter()
        .try_fold(1i64, |n, length| n.checked_mul(length?))
}

pub(super) fn mset_size(attrs: &MSetAttr<IntVal>, inner: &DomainPtr) -> SizeBounds {
    let size = SizeBounds::from_size(&attrs.size);
    let occurrence = SizeBounds::from_size(&attrs.occurrence);
    let capacity = match (capacity(inner), occurrence.high) {
        (Some(0), _) | (_, Some(0)) => Some(0),
        (Some(n), Some(occ)) => n.checked_mul(occ),
        _ => None,
    };
    size.cap(capacity)
}

pub(super) fn collection_size(domain: &DomainPtr) -> Option<SizeBounds> {
    if let Some((attrs, inner)) = domain.as_set() {
        Some(SizeBounds::from_size(&attrs.size).cap(capacity(&inner)))
    } else if let Some((attrs, inner)) = domain.as_mset() {
        Some(mset_size(&attrs, &inner))
    } else if let Some((_, dimensions)) = domain.as_matrix() {
        // Matrix cardinality counts top-level entries, including rows of a higher-rank matrix.
        Some(match dimensions.first().and_then(capacity) {
            Some(n) => SizeBounds::new(n, Some(n)),
            None => SizeBounds::new(0, None),
        })
    } else if let Some((attrs, domains)) = domain.as_relation() {
        Some(SizeBounds::from_size(&attrs.size).cap(product_capacity(&domains)))
    } else if let Some((attrs, inner)) = domain.as_sequence() {
        let mut bounds = SizeBounds::from_size(&attrs.size);
        let n = capacity(&inner);
        match attrs.jectivity {
            JectivityAttr::Injective => bounds = bounds.cap(n),
            JectivityAttr::Surjective => bounds.low = bounds.low.max(n.unwrap_or(0)),
            JectivityAttr::Bijective => {
                bounds.low = bounds.low.max(n.unwrap_or(0));
                bounds = bounds.cap(n);
            }
            JectivityAttr::None => {}
        }
        if n == Some(0) {
            bounds = bounds.cap(Some(0));
        }
        Some(bounds)
    } else {
        None
    }
}

/// Duplicates can shrink a collection; each present value occurs at least once.
pub(super) fn distinct_size(size: SizeBounds, occurrence: SizeBounds) -> SizeBounds {
    let low = if size.low == 0 {
        0
    } else {
        occurrence
            .high
            .filter(|n| *n > 0)
            .map(|n| 1 + (size.low - 1) / n)
            .unwrap_or(1)
    };
    let high = size.high.map(|n| n / occurrence.low.max(1));
    SizeBounds::new(low, high)
}

/// Count integer values using interval arithmetic, including ranges wider than i32.
pub(super) fn capacity(domain: &DomainPtr) -> Option<i64> {
    ground_capacity(domain.resolve().ok()?.as_ref())
}

fn ground_capacity(ground: &GroundDomain) -> Option<i64> {
    if let GroundDomain::Int(ranges, _) = ground {
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
            GroundDomain::Tuple(domains, _) => domains
                .iter()
                .try_fold(1i64, |n, d| n.checked_mul(ground_capacity(d)?)),
            GroundDomain::Record(fields, _) => fields
                .iter()
                .try_fold(1i64, |n, f| n.checked_mul(ground_capacity(&f.value)?)),
            GroundDomain::Variant(fields, _) => fields
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
            (GroundDomain::Int(a_ranges, _), GroundDomain::Int(b_ranges, _)) => {
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
    let a_rep = super::expression_domains::selected_representation(a).or(a_attr.representation);
    let b_rep = super::expression_domains::selected_representation(b).or(b_attr.representation);
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
    #[test]
    fn multiset_cardinality_and_distinct_bounds_cover_small_values() {
        for min_size in 0..=5 {
            for max_size in min_size..=6 {
                for min_occurrence in 1..=3 {
                    for max_occurrence in min_occurrence..=3 {
                        let attrs = MSetAttr::new(
                            Range::Bounded(min_size, max_size),
                            Range::Bounded(min_occurrence, max_occurrence),
                        );
                        let domain = Domain::mset(attrs, Domain::int(vec![Range::Bounded(0, 2)]));
                        let source = variable("m", domain);
                        let card = Expression::Card(Metadata::new(), Moo::new(source.clone()))
                            .domain_of()
                            .unwrap()
                            .resolve()
                            .unwrap();
                        let set = Expression::ToSet(Metadata::new(), Moo::new(source))
                            .domain_of()
                            .unwrap()
                            .resolve()
                            .unwrap();
                        for a in 0..=3 {
                            for b in 0..=3 {
                                for c in 0..=3 {
                                    let counts = [a, b, c];
                                    let n = a + b + c;
                                    if !(min_size..=max_size).contains(&n)
                                        || counts.iter().any(|x| {
                                            *x != 0
                                                && !(min_occurrence..=max_occurrence).contains(x)
                                        })
                                    {
                                        continue;
                                    }
                                    let elems = counts
                                        .iter()
                                        .enumerate()
                                        .filter(|(_, n)| **n > 0)
                                        .map(|(i, _)| Literal::Int(i as i32))
                                        .collect();
                                    assert!(card.contains(&Literal::Int(n)).unwrap());
                                    assert!(
                                        set.contains(&Literal::AbstractLiteral(
                                            AbstractLiteral::Set(elems)
                                        ))
                                        .unwrap()
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
    fn multiset_union_retains_one_sided_bounds() {
        let a = variable(
            "a",
            Domain::mset(
                MSetAttr::new_min_size(2),
                Domain::int(vec![Range::Bounded(0, 2)]),
            ),
        );
        let b = variable(
            "b",
            Domain::mset(MSetAttr::new_size(1), Domain::int(vec![Range::Single(3)])),
        );
        let result = operation(&a, &b, SetOperation::Union).domain_of().unwrap();
        let (attrs, _) = result.as_mset_ground().unwrap();
        assert_eq!(attrs.size, Range::UnboundedR(3));
        assert_eq!(attrs.occurrence, Range::UnboundedR(1));
    }

    #[test]
    fn multiset_union_accepts_an_untyped_empty_literal() {
        let a = variable(
            "a",
            Domain::mset(MSetAttr::new_size(2), Domain::int(vec![Range::Single(1)])),
        );
        let empty = Expression::AbstractLiteral(Metadata::new(), AbstractLiteral::MSet(vec![]));
        for (a, b) in [(&a, &empty), (&empty, &a)] {
            let result = operation(a, b, SetOperation::Union).domain_of().unwrap();
            assert_eq!(result.as_mset_ground().unwrap().0.size, Range::Single(2));
        }
    }

    #[test]
    fn bounds_wider_than_i32_weaken_without_becoming_empty() {
        let max = i64::from(i32::MAX);
        assert_eq!(
            SizeBounds::new(max + 1, Some(max * 2)).range(),
            Some(Range::UnboundedR(i32::MAX))
        );
        let low = Domain::int(vec![
            Range::Bounded(i32::MIN, 0),
            Range::Bounded(i32::MIN, -1),
        ]);
        assert_eq!(capacity(&low), Some(max + 2));
    }

    #[test]
    fn constant_literal_sets_count_distinct_entries_and_preserve_empty_values() {
        use crate::ast::HasDomain;
        let duplicate =
            Literal::AbstractLiteral(AbstractLiteral::Set(vec![Literal::Int(1), Literal::Int(1)]));
        let domain = duplicate.domain_of();
        assert_eq!(domain.as_set_ground().unwrap().0.size, Range::Single(1));
        assert!(domain.resolve().unwrap().contains(&duplicate).unwrap());
        let normalised = crate::ast::eval_constant(&Expression::from(duplicate)).unwrap();
        assert!(domain.resolve().unwrap().contains(&normalised).unwrap());
        let value =
            Literal::AbstractLiteral(AbstractLiteral::Set(vec![Literal::Int(1), Literal::Int(2)]));
        let domain = value.domain_of();
        assert_eq!(domain.as_set_ground().unwrap().0.size, Range::Single(2));
        assert!(domain.resolve().unwrap().contains(&value).unwrap());
        let empty = Expression::AbstractLiteral(Metadata::new(), AbstractLiteral::Set(vec![]));
        let a = variable(
            "a",
            Domain::set(SetAttr::new_size(1), Domain::int(vec![Range::Single(1)])),
        );
        let result = operation(&a, &empty, SetOperation::Union)
            .domain_of()
            .unwrap();
        assert_eq!(result.as_set_ground().unwrap().0.size, Range::Single(1));
    }

    #[test]
    fn sequence_attributes_bound_cardinality_but_repetition_has_no_support_cap() {
        use crate::ast::SequenceAttr;
        for (jectivity, expected) in [
            (JectivityAttr::None, Range::UnboundedR(0)),
            (JectivityAttr::Injective, Range::Bounded(0, 3)),
            (JectivityAttr::Surjective, Range::UnboundedR(3)),
            (JectivityAttr::Bijective, Range::Single(3)),
        ] {
            let source = variable(
                "s",
                Domain::sequence(
                    SequenceAttr::<i32> {
                        jectivity,
                        ..Default::default()
                    },
                    Domain::int(vec![Range::Bounded(0, 2)]),
                ),
            );
            assert_eq!(
                Expression::Card(Metadata::new(), Moo::new(source)).domain_of(),
                Some(Domain::int(vec![expected]))
            );
        }
    }

    #[test]
    fn matrix_to_set_uses_element_type_and_length_bounds() {
        let source = variable(
            "xs",
            Domain::matrix(
                Domain::int(vec![Range::Bounded(0, 1)]),
                vec![Domain::int(vec![Range::Bounded(1, 3)])],
            ),
        );
        let result = Expression::ToSet(Metadata::new(), Moo::new(source))
            .domain_of()
            .unwrap();
        let (attrs, inner) = result.as_set_ground().unwrap();
        assert_eq!(attrs.size, Range::Bounded(1, 2));
        assert_eq!(inner.return_type(), ReturnType::Int);
    }
    #[test]
    fn matrix_cardinality_domain_matches_every_two_dimensional_value() {
        let domain = Domain::matrix(
            Domain::bool(),
            vec![
                Domain::int(vec![Range::Bounded(1, 2)]),
                Domain::int(vec![Range::Bounded(1, 3)]),
            ],
        );
        let source = variable("matrix", domain.clone());
        let inferred = Expression::Card(Metadata::new(), Moo::new(source))
            .domain_of()
            .unwrap();
        assert_eq!(inferred, Domain::int(vec![Range::Single(2)]));
        for value in domain.resolve().unwrap().values().unwrap() {
            let actual = crate::ast::eval_constant(&Expression::Card(
                Metadata::new(),
                Moo::new(value.into()),
            ))
            .unwrap();
            assert!(inferred.resolve().unwrap().contains(&actual).unwrap());
        }
    }
}
