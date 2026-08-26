//! Helpers shared by the SAT integer representations.
//!
//! The three encodings differ in how they lay out bits, but agree on which domains they accept
//! and on how a domain is restated as a constraint over the encoded value.

use conjure_cp::ast::{DomainPtr, Expression, GroundDomain, Metadata, Moo, Range};
use conjure_cp::essence_expr;
use conjure_cp::into_matrix_expr;

/// The inclusive intervals of a finite ground integer domain.
///
/// Returns `None` for anything that is not an integer domain, and for integer domains with an
/// unbounded range: every SAT encoding needs to know how many values it is laying out.
pub(crate) fn int_ranges(dom: &DomainPtr) -> Option<Vec<(i32, i32)>> {
    let GroundDomain::Int(ranges) = dom.as_ground()? else {
        return None;
    };

    ranges
        .iter()
        .map(|range| Some((*range.low()?, *range.high()?)))
        .collect()
}

/// The overall lower and upper bound of a set of inclusive intervals.
pub(crate) fn finite_int_bounds(ranges: &[(i32, i32)]) -> Option<(i32, i32)> {
    let low = ranges.iter().map(|(low, _)| *low).min()?;
    let high = ranges.iter().map(|(_, high)| *high).max()?;
    Some((low, high))
}

/// Restate an integer domain as a constraint on `subject`, the encoded form of the variable.
///
/// The encodings all give the solver a value in `low..=high`; this is what rules out the gaps in
/// between, and for the log encoding also the two's-complement values outside the domain entirely.
pub(crate) fn int_domain_to_expr(subject: Expression, ranges: &[(i32, i32)]) -> Expression {
    let subject = Moo::new(subject);
    let constraints = ranges
        .iter()
        .map(|(low, high)| {
            if low == high {
                essence_expr!(&subject = &low)
            } else {
                essence_expr!(r"&subject >= &low /\ &subject <= &high")
            }
        })
        .collect();

    Expression::Or(Metadata::new(), Moo::new(into_matrix_expr!(constraints)))
}

/// The `Range` list of a ground integer domain, for callers that need Essence ranges back.
#[allow(dead_code)]
pub(crate) fn as_ranges(bounds: &[(i32, i32)]) -> Vec<Range<i32>> {
    bounds
        .iter()
        .map(|(low, high)| {
            if low == high {
                Range::Single(*low)
            } else {
                Range::Bounded(*low, *high)
            }
        })
        .collect()
}
