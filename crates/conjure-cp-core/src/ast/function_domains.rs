//! Function attributes constrain mappings, distinct images, and inverse images differently.

use super::collection_domains::{SizeBounds, bounded_set, capacity, min_known, overlap_domain};
use super::{
    Domain, DomainPtr, Expression, FuncAttr, GroundDomain, IntVal, JectivityAttr, PartialityAttr,
    ReturnType, Typeable, eval_constant,
};

pub(super) fn function_size(
    attrs: &FuncAttr<IntVal>,
    domain: &DomainPtr,
    codomain: &DomainPtr,
) -> SizeBounds {
    let mut size = SizeBounds::from_size(&attrs.size).cap(capacity(domain));
    if attrs.partiality == PartialityAttr::Total {
        size.low = size.low.max(capacity(domain).unwrap_or(0));
    }
    let codomain_size = capacity(codomain);
    if matches!(
        attrs.jectivity,
        JectivityAttr::Injective | JectivityAttr::Bijective
    ) {
        size = size.cap(codomain_size);
    }
    if matches!(
        attrs.jectivity,
        JectivityAttr::Surjective | JectivityAttr::Bijective
    ) {
        size.low = size.low.max(codomain_size.unwrap_or(0));
    }
    if codomain_size == Some(0) {
        size = size.cap(Some(0));
    }
    size
}

fn empty_set(inner: &DomainPtr) -> DomainPtr {
    Domain::empty(ReturnType::Set(Box::new(inner.return_type())))
}

pub(super) fn defined_domain(function: &Expression) -> Option<DomainPtr> {
    let (attrs, domain, codomain) = function.domain_of()?.as_function()?;
    Some(bounded_set(
        function_size(&attrs, &domain, &codomain),
        domain,
    ))
}

pub(super) fn range_domain(function: &Expression) -> Option<DomainPtr> {
    let (attrs, domain, codomain) = function.domain_of()?.as_function()?;
    let size = function_size(&attrs, &domain, &codomain);
    if size.range().is_none() {
        return Some(empty_set(&codomain));
    }
    let bounds = match attrs.jectivity {
        JectivityAttr::Injective | JectivityAttr::Bijective => size,
        JectivityAttr::Surjective => match capacity(&codomain) {
            Some(n) => SizeBounds::new(n, Some(n)),
            None => SizeBounds::new(i64::from(size.low > 0), size.high),
        },
        JectivityAttr::None => SizeBounds::new(i64::from(size.low > 0), size.high),
    };
    Some(bounded_set(bounds, codomain))
}

/// Only exact intersections can prove containment or how many keys a restriction retains.
fn exact_overlap(a: &DomainPtr, b: &DomainPtr) -> bool {
    if a == b {
        return true;
    }
    match (a.resolve(), b.resolve()) {
        (Ok(a), Ok(b)) => {
            a == b
                || matches!(
                    (a.as_ref(), b.as_ref()),
                    (GroundDomain::Int(_, _), GroundDomain::Int(_, _))
                        | (GroundDomain::Empty(_), _)
                        | (_, GroundDomain::Empty(_))
                )
        }
        _ => false,
    }
}

fn within(a: &DomainPtr, b: &DomainPtr) -> bool {
    if a == b {
        return true;
    }
    if !exact_overlap(a, b) {
        return false;
    }
    capacity(a).is_some_and(|n| capacity(&overlap_domain(a, b)) == Some(n))
}

pub(super) fn pre_image_domain(function: &Expression, image: &Expression) -> Option<DomainPtr> {
    let (attrs, domain, codomain) = function.domain_of()?.as_function()?;
    let size = function_size(&attrs, &domain, &codomain);
    let image_domain = image.domain_of()?;
    if size.range().is_none() || capacity(&image_domain) == Some(0) {
        return Some(empty_set(&domain));
    }
    if capacity(&overlap_domain(&image_domain, &codomain)) == Some(0) {
        return Some(bounded_set(SizeBounds::new(0, Some(0)), domain));
    }
    let is_within = eval_constant(image)
        .and_then(|value| codomain.resolve().ok()?.contains(&value).ok())
        .unwrap_or_else(|| within(&image_domain, &codomain));
    let injective = matches!(
        attrs.jectivity,
        JectivityAttr::Injective | JectivityAttr::Bijective
    );
    let surjective = matches!(
        attrs.jectivity,
        JectivityAttr::Surjective | JectivityAttr::Bijective
    );
    let mut low = i64::from(surjective && is_within);
    let mut high = size.high;
    if injective {
        high = min_known(high, Some(1));
    }
    if surjective && let Some(n) = capacity(&codomain) {
        high = min_known(high, size.high.map(|max| (max - n + 1).max(0)));
    }
    // With a singleton codomain all mappings have the queried image.
    if is_within && capacity(&codomain) == Some(1) {
        low = size.low;
    }
    Some(bounded_set(SizeBounds::new(low, high), domain))
}

pub(super) fn image_set_domain(function: &Expression, argument: &Expression) -> Option<DomainPtr> {
    let (attrs, domain, codomain) = function.domain_of()?.as_function()?;
    let size = function_size(&attrs, &domain, &codomain);
    let argument_domain = argument.domain_of()?;
    if size.range().is_none() || capacity(&argument_domain) == Some(0) {
        return Some(empty_set(&codomain));
    }
    if size.high == Some(0) || capacity(&overlap_domain(&argument_domain, &domain)) == Some(0) {
        return Some(bounded_set(SizeBounds::new(0, Some(0)), codomain));
    }
    let is_within = eval_constant(argument)
        .and_then(|value| domain.resolve().ok()?.contains(&value).ok())
        .unwrap_or_else(|| within(&argument_domain, &domain));
    let total = attrs.partiality == PartialityAttr::Total
        || capacity(&domain).is_some_and(|n| size.low == n);
    Some(bounded_set(
        SizeBounds::new(i64::from(total && is_within), Some(1)),
        codomain,
    ))
}

pub(super) fn restricted_domain(
    function: &Expression,
    restriction: &Expression,
) -> Option<DomainPtr> {
    let (attrs, domain, codomain) = function.domain_of()?.as_function()?;
    let requested = restriction.domain_of()?;
    let retained = overlap_domain(&domain, &requested);
    let source_size = function_size(&attrs, &domain, &codomain);
    if source_size.range().is_none() {
        return Some(Domain::empty(ReturnType::Function(
            Box::new(domain.return_type()),
            Box::new(codomain.return_type()),
        )));
    }
    let unchanged = within(&domain, &requested);
    let exact = exact_overlap(&domain, &requested);
    let low = if unchanged {
        source_size.low
    } else if exact {
        capacity(&domain)
            .zip(capacity(&retained))
            .map(|(old, new)| (source_size.low - (old - new)).max(0))
            .unwrap_or(0)
    } else {
        0
    };
    let size = SizeBounds::new(low, source_size.high)
        .cap(capacity(&retained))
        .range()?;
    let partiality = if exact {
        attrs.partiality
    } else {
        PartialityAttr::Partial
    };
    let jectivity = match attrs.jectivity {
        JectivityAttr::Bijective if !unchanged => JectivityAttr::Injective,
        JectivityAttr::Surjective if !unchanged => JectivityAttr::None,
        other => other,
    };
    Some(Domain::function(
        FuncAttr {
            size,
            partiality,
            jectivity,
            representation: attrs.representation.clone(),
        },
        retained,
        codomain,
    ))
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::ast::{
        AbstractLiteral, DeclarationPtr, Literal, Metadata, Moo, Name, Range, Reference,
    };
    use std::collections::BTreeSet;

    fn variable(name: &str, domain: DomainPtr) -> Expression {
        Reference::new(DeclarationPtr::new_find(Name::user(name), domain)).into()
    }

    fn ints(n: i32) -> DomainPtr {
        Domain::int((0..n).map(Range::Single).collect())
    }

    fn set(values: impl Iterator<Item = i32>) -> Literal {
        Literal::AbstractLiteral(AbstractLiteral::Set(
            values
                .collect::<BTreeSet<_>>()
                .into_iter()
                .map(Literal::Int)
                .collect(),
        ))
    }

    #[test]
    fn function_inference_covers_every_small_mapping() {
        for domain_len in 0..=3 {
            for codomain_len in 0..=3 {
                for partiality in [PartialityAttr::Partial, PartialityAttr::Total] {
                    for jectivity in [
                        JectivityAttr::None,
                        JectivityAttr::Injective,
                        JectivityAttr::Surjective,
                        JectivityAttr::Bijective,
                    ] {
                        for size in [
                            Range::Unbounded,
                            Range::Single(0),
                            Range::Single(1),
                            Range::UnboundedR(2),
                            Range::UnboundedL(2),
                            Range::Bounded(1, 2),
                        ] {
                            let source = variable(
                                "f",
                                Domain::function(
                                    FuncAttr {
                                        size: size.clone(),
                                        partiality: partiality.clone(),
                                        jectivity: jectivity.clone(),
                                        representation: None,
                                    },
                                    ints(domain_len),
                                    ints(codomain_len),
                                ),
                            );
                            let defined = defined_domain(&source).unwrap().resolve().unwrap();
                            let range = range_domain(&source).unwrap().resolve().unwrap();
                            let cardinality =
                                Expression::Card(Metadata::new(), Moo::new(source.clone()))
                                    .domain_of()
                                    .unwrap()
                                    .resolve()
                                    .unwrap();
                            let images = (-1..=codomain_len)
                                .map(|image| {
                                    (
                                        image,
                                        pre_image_domain(&source, &Literal::Int(image).into())
                                            .unwrap()
                                            .resolve()
                                            .unwrap(),
                                    )
                                })
                                .collect::<Vec<_>>();
                            let restrictions = (0i32..1 << domain_len)
                                .map(|mask| {
                                    let target = Domain::int(
                                        (0..domain_len)
                                            .filter(|i| mask & (1 << i) != 0)
                                            .map(Range::Single)
                                            .collect(),
                                    );
                                    (
                                        mask,
                                        restricted_domain(&source, &variable("D", target))
                                            .unwrap()
                                            .resolve()
                                            .unwrap(),
                                    )
                                })
                                .collect::<Vec<_>>();
                            for mut code in 0..(codomain_len + 1).pow(domain_len as u32) {
                                let mut pairs = Vec::new();
                                for key in 0..domain_len {
                                    let value = code % (codomain_len + 1);
                                    code /= codomain_len + 1;
                                    if value > 0 {
                                        pairs.push((key, value - 1));
                                    }
                                }
                                let n = pairs.len() as i32;
                                if !size.contains(&n)
                                    || (partiality == PartialityAttr::Total && n != domain_len)
                                {
                                    continue;
                                }
                                let distinct =
                                    pairs.iter().map(|(_, v)| v).collect::<BTreeSet<_>>().len()
                                        as i32;
                                if matches!(
                                    jectivity,
                                    JectivityAttr::Injective | JectivityAttr::Bijective
                                ) && distinct != n
                                {
                                    continue;
                                }
                                if matches!(
                                    jectivity,
                                    JectivityAttr::Surjective | JectivityAttr::Bijective
                                ) && distinct != codomain_len
                                {
                                    continue;
                                }
                                assert!(
                                    defined
                                        .contains(&set(pairs.iter().map(|(k, _)| *k)))
                                        .unwrap(),
                                    "defined({source}): {pairs:?} outside {defined}"
                                );
                                assert!(
                                    range.contains(&set(pairs.iter().map(|(_, v)| *v))).unwrap(),
                                    "range({source}): {pairs:?} outside {range}"
                                );
                                assert!(cardinality.contains(&Literal::Int(n)).unwrap());
                                for (image, domain) in &images {
                                    assert!(
                                        domain
                                            .contains(&set(pairs
                                                .iter()
                                                .filter(|(_, v)| v == image)
                                                .map(|(k, _)| *k)))
                                            .unwrap(),
                                        "preImage({source},{image}): {pairs:?} outside {domain}"
                                    );
                                }
                                for (mask, domain) in &restrictions {
                                    let pairs = pairs
                                        .iter()
                                        .filter(|(k, _)| mask & (1 << k) != 0)
                                        .map(|(k, v)| (Literal::Int(*k), Literal::Int(*v)))
                                        .collect::<Vec<_>>();
                                    let value =
                                        Literal::AbstractLiteral(AbstractLiteral::Function(pairs));
                                    assert!(
                                        domain.contains(&value).unwrap(),
                                        "restrict({source},{mask}): {value} outside {domain}"
                                    );
                                    let GroundDomain::Function(attrs, _, _) = domain.as_ref()
                                    else {
                                        unreachable!()
                                    };
                                    let Literal::AbstractLiteral(AbstractLiteral::Function(pairs)) =
                                        value
                                    else {
                                        unreachable!()
                                    };
                                    if attrs.partiality == PartialityAttr::Total {
                                        assert_eq!(pairs.len() as u32, mask.count_ones());
                                    }
                                    if matches!(
                                        attrs.jectivity,
                                        JectivityAttr::Surjective | JectivityAttr::Bijective
                                    ) {
                                        let mut images =
                                            pairs.iter().map(|(_, v)| v).collect::<Vec<_>>();
                                        images.sort_by(|a, b| a.essence_cmp(b));
                                        images.dedup();
                                        assert_eq!(images.len() as i32, codomain_len);
                                    }
                                }
                            }
                        }
                    }
                }
            }
        }
    }

    #[test]
    fn restriction_drops_surjectivity_and_reduces_minimum_size() {
        let attrs = FuncAttr {
            size: Range::Single(8),
            partiality: PartialityAttr::Partial,
            jectivity: JectivityAttr::Surjective,
            representation: None,
        };
        let source = variable("f", Domain::function(attrs, ints(10), ints(3)));
        let result = restricted_domain(&source, &variable("D", ints(5))).unwrap();
        let (attrs, _, _) = result.as_function().unwrap();
        assert_eq!(attrs.size.resolve().unwrap(), Range::Bounded(3, 5));
        assert_eq!(attrs.jectivity, JectivityAttr::None);
    }

    #[test]
    fn preimage_query_can_be_outside_a_bijective_codomain() {
        let source = variable(
            "f",
            Domain::function(
                FuncAttr {
                    size: Range::<i32>::Unbounded,
                    partiality: PartialityAttr::Total,
                    jectivity: JectivityAttr::Bijective,
                    representation: None,
                },
                ints(2),
                ints(2),
            ),
        );
        let outside = pre_image_domain(&source, &Literal::Int(2).into()).unwrap();
        assert_eq!(outside.as_set_ground().unwrap().0.size, Range::Single(0));
        let query = variable("q", Domain::int(vec![Range::Bounded(0, 2)]));
        let mixed = pre_image_domain(&source, &query).unwrap();
        assert_eq!(mixed.as_set_ground().unwrap().0.size, Range::Bounded(0, 1));
    }

    #[test]
    fn image_set_is_singleton_only_for_guaranteed_defined_arguments() {
        let source = variable(
            "f",
            Domain::function(
                FuncAttr {
                    size: Range::<i32>::Unbounded,
                    partiality: PartialityAttr::Total,
                    jectivity: JectivityAttr::None,
                    representation: None,
                },
                ints(2),
                ints(3),
            ),
        );
        let inside = image_set_domain(&source, &Literal::Int(0).into()).unwrap();
        assert_eq!(inside.as_set_ground().unwrap().0.size, Range::Single(1));
        let outside = image_set_domain(&source, &Literal::Int(2).into()).unwrap();
        assert_eq!(outside.as_set_ground().unwrap().0.size, Range::Single(0));
    }

    #[test]
    fn symbolic_restriction_does_not_preserve_totality_or_surjectivity() {
        let n = IntVal::Reference(Reference::new(DeclarationPtr::new_given(
            Name::user("n"),
            ints(4),
        )));
        let target = Domain::int(vec![Range::Bounded(IntVal::from(0), n)]);
        let source = variable(
            "f",
            Domain::function(
                FuncAttr {
                    size: Range::<i32>::Unbounded,
                    partiality: PartialityAttr::Total,
                    jectivity: JectivityAttr::Bijective,
                    representation: None,
                },
                ints(3),
                ints(3),
            ),
        );
        let result = restricted_domain(&source, &variable("D", target)).unwrap();
        let (attrs, _, _) = result.as_function().unwrap();
        assert_eq!(attrs.size.resolve().unwrap(), Range::Bounded(0, 3));
        assert_eq!(attrs.partiality, PartialityAttr::Partial);
        assert_eq!(attrs.jectivity, JectivityAttr::Injective);
    }
}
