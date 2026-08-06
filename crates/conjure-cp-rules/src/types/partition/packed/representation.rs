//! A packed channelling for partitions of a small, fully enumerable inner domain: the whole
//! `WhichPart` array (one part-number digit per inner-domain element, in canonical first-
//! occurrence order -- see `PartitionOccurrence`'s own doc comment for why that ordering breaks
//! the part-labelling symmetry) is encoded as a single mixed-radix integer, one digit per element,
//! every digit sharing the same radix (`maxNumParts`). `NumParts` and `PartSizes` stay as separate,
//! small aux variables exactly as in `PartitionOccurrence` -- they are already compact (bounded by
//! `maxNumParts`, not by the inner domain's own size), so packing them too would not shrink the
//! representation further, and keeping them separate lets every part-size/regular structural
//! constraint be reused unchanged from `PartitionOccurrence` via `partition::common`.
//!
//! Unlike `PartitionOccurrence`, the inner domain only needs to be enumerable (any finite domain
//! `PartitionAsSet` itself would also accept), not specifically matrix-indexable, since packing
//! never builds a native multi-dimensional matrix -- closer to `RelationPacked`'s own scope than
//! `RelationOccurrence`'s.

use crate::shared::representation_prelude::*;
use crate::types::partition::common::{
    eq, gt, implies, index1, int_literal, part_size_cons_expr, regular_expr,
    resolve_partition_size_attrs,
};
use conjure_cp::ast::{Domain, GroundDomain, Moo, Range, Reference};
use conjure_cp::{domain_int, essence_expr, into_matrix_expr, range};

register_representation!(
    PartitionPacked("packed")
    struct State<T> {
        /// The complete `WhichPart` array encoded as one mixed-radix integer: digit `i` (0-based)
        /// says which part `elements[i]` belongs to (0-based; add 1 for the 1-based part number
        /// used everywhere else, matching `PartitionOccurrence`'s own convention).
        pub packed: T,
        /// Number of active parts.
        pub num_parts_decl: T,
        /// For each possible part number, how many elements are in it.
        pub part_sizes_decl: T,
        /// Inner-domain values in canonical (`values()`) order; digit `i` of `packed` says which
        /// part `elements[i]` belongs to.
        pub elements: Moo<Vec<Literal>>,
        /// `int(1..maxNumParts)`, shared by `part_sizes_decl`'s index domain and every structural
        /// constraint that quantifies over "all possible part numbers".
        pub part_index_domain: DomainPtr,
        /// The largest number of parts this partition could ever have.
        pub max_num_parts: i32,
        /// The (already bounded, and `regular`-inferred where applicable) part-size attribute.
        pub part_len: Range<i32>,
        /// Whether every part must have equal cardinality.
        pub is_regular: bool,
        /// Whether `partSize` already fixes an exact part size, in which case `regular` is
        /// automatically implied and stating it again would be redundant.
        pub fixed_part_size: bool,
        /// Number of packed states, including cardinality/attribute-invalid ones (`maxNumParts ^
        /// |elements|`).
        pub total_size: i32
    }
    impl State<DeclarationPtr> {
        pub fn packed_expr(&self) -> Expression {
            Reference::new(self.packed.clone()).into()
        }

        /// The 0-based digit for inner-domain position `index`.
        fn digit_expr(&self, index: usize) -> Expression {
            let packed = self.packed_expr();
            let place = self.max_num_parts.pow((self.elements.len() - 1 - index) as u32);
            let radix = self.max_num_parts;
            match (place, index) {
                (1, 0) => packed,
                (1, _) => essence_expr!(&packed % &radix),
                (_, 0) => essence_expr!(&packed / &place),
                (_, _) => essence_expr!((&packed / &place) % &radix),
            }
        }

        /// The 1-based part number for inner-domain position `index`.
        pub fn part_number_expr(&self, index: usize) -> Expression {
            let digit = self.digit_expr(index);
            essence_expr!(&digit + 1)
        }
    }
    impl<T> State<T> {
        /// Encode a full assignment of (0-based) part-number digits, one per element, into a
        /// packed rank.
        fn encode(&self, digits: &[i32]) -> Option<i32> {
            if digits.len() != self.elements.len() {
                return None;
            }
            digits.iter().try_fold(0i32, |packed, digit| {
                packed
                    .checked_mul(self.max_num_parts)?
                    .checked_add(*digit)
            })
        }
    }
    fn init(dom: DomainPtr) -> Result<State<DomainPtr>, ReprInitError> {
        let domain_err = |msg: &str| ReprInitError::UnsupportedDomain(
            dom.clone(),
            PartitionPacked::NAME,
            String::from(msg),
        );
        let Some(GroundDomain::Partition(attr, inner)) = dom.as_ground() else {
            return Err(domain_err("expected a ground partition domain"));
        };
        let inner_domain: DomainPtr = inner.clone().into();
        let max_size = inner_domain
            .length_signed()
            .map_err(|_| domain_err("inner domain must have a known finite size"))?;
        let (num_parts, part_len, fixed_part_size) = resolve_partition_size_attrs(attr, max_size);
        let max_num_parts = num_parts.high().copied().unwrap_or(max_size).max(1);
        let max_part_size = part_len.high().copied().unwrap_or(max_size).max(0);

        let elements: Vec<Literal> = inner_domain
            .values()
            .map_err(|e| domain_err(&format!("could not enumerate the inner domain: {e}")))?
            .collect();
        let total_size = max_num_parts
            .checked_pow(u32::try_from(elements.len()).map_err(|_| domain_err("inner domain is too large to pack"))?)
            .ok_or_else(|| domain_err("packed partition domain would overflow i32"))?;

        let part_index_domain = Domain::int(vec![Range::new(Some(1), Some(max_num_parts))]);
        let part_sizes_decl = Domain::matrix(
            Domain::int(vec![Range::new(Some(0), Some(max_part_size))]),
            vec![part_index_domain.clone()],
        );

        Ok(State {
            packed: domain_int!(0..(total_size - 1)),
            num_parts_decl: Domain::int(vec![num_parts]),
            part_sizes_decl,
            elements: Moo::new(elements),
            part_index_domain,
            max_num_parts,
            part_len,
            is_regular: attr.is_regular,
            fixed_part_size,
            total_size,
        })
    }
    fn structural(state: &State<DeclarationPtr>) -> Vec<Expression> {
        let part_sizes_ref: Expression = Reference::new(state.part_sizes_decl.clone()).into();
        let num_parts_ref: Expression = Reference::new(state.num_parts_decl.clone()).into();
        let part_numbers: Vec<Expression> = (0..state.elements.len())
            .map(|i| state.part_number_expr(i))
            .collect();

        let mut constraints = vec![
            bound_which_part_expr(&part_numbers, &num_parts_ref),
            parts_nonempty_expr(&part_numbers, &state.part_index_domain, &num_parts_ref),
            part_sizes_channelling_expr(&part_numbers, &state.part_index_domain, &part_sizes_ref),
            part_size_cons_expr(
                &state.part_index_domain,
                &part_sizes_ref,
                &num_parts_ref,
                &state.part_len,
            ),
            symmetry_breaking_expr(&part_numbers),
        ];

        if state.is_regular && !state.fixed_part_size {
            let regular_index_domain = if state.max_num_parts >= 2 {
                Domain::int(vec![Range::new(Some(2), Some(state.max_num_parts))])
            } else {
                Domain::int(Vec::<Range<i32>>::new())
            };
            constraints.push(regular_expr(&regular_index_domain, &part_sizes_ref, &num_parts_ref));
        }

        constraints
    }
    fn down(state: &State<DomainPtr>, value: Literal) -> Result<State<Literal>, ReprDownError> {
        let Literal::AbstractLiteral(AbstractLiteral::Partition(parts)) = value else {
            return Err(ReprDownError::BadValue(value, String::from("expected a partition literal")));
        };
        if parts.len() as i32 > state.max_num_parts {
            return Err(ReprDownError::BadValue(
                AbstractLiteral::Partition(parts).into(),
                format!("expected at most {} parts", state.max_num_parts),
            ));
        }

        // Canonical (first-occurrence) labelling, matching `PartitionOccurrence::down` exactly --
        // this must agree with `symmetry_breaking_expr`'s own ordering, or a valid partition
        // literal could end up failing the very constraint meant to allow it.
        let mut part_label: Vec<Option<i32>> = vec![None; parts.len()];
        let mut digits: Vec<i32> = Vec::with_capacity(state.elements.len());
        let mut next_label = 1i32;
        for value in state.elements.iter() {
            let owner = parts
                .iter()
                .position(|part| part.iter().any(|v| v.essence_cmp(value).is_eq()))
                .ok_or_else(|| ReprDownError::BadValue(
                    AbstractLiteral::Partition(parts.clone()).into(),
                    format!("{value} does not appear in any part"),
                ))?;
            let label = match part_label[owner] {
                Some(label) => label,
                None => {
                    let label = next_label;
                    part_label[owner] = Some(label);
                    next_label += 1;
                    label
                }
            };
            digits.push(label - 1);
        }
        let num_parts_val = next_label - 1;

        let mut part_sizes_vals = vec![0i32; state.max_num_parts as usize];
        for (owner, label) in part_label.iter().enumerate() {
            if let Some(label) = label {
                part_sizes_vals[(*label - 1) as usize] = parts[owner].len() as i32;
            }
        }

        let packed = state.encode(&digits).ok_or_else(|| ReprDownError::BadValue(
            AbstractLiteral::Partition(parts.clone()).into(),
            String::from("partition value is outside its representation domain"),
        ))?;

        Ok(State {
            packed: Literal::Int(packed),
            num_parts_decl: Literal::Int(num_parts_val),
            part_sizes_decl: Literal::AbstractLiteral(AbstractLiteral::Matrix(
                part_sizes_vals.into_iter().map(Literal::Int).collect(),
                Moo::new(GroundDomain::Int(vec![Range::new(Some(1), Some(state.max_num_parts))])),
            )),
            elements: state.elements.clone(),
            part_index_domain: state.part_index_domain.clone(),
            max_num_parts: state.max_num_parts,
            part_len: state.part_len.clone(),
            is_regular: state.is_regular,
            fixed_part_size: state.fixed_part_size,
            total_size: state.total_size,
        })
    }
    fn up(state: State<Literal>) -> Literal {
        let Literal::Int(packed) = state.packed else {
            bug!("expected a packed partition integer, got {}", state.packed)
        };
        if packed < 0 || packed >= state.total_size {
            bug!("packed partition value {packed} is outside its representation domain");
        }
        let n = state.elements.len();
        let mut parts: Vec<Vec<Literal>> = Vec::new();
        let mut remaining = packed;
        let mut digits = vec![0i32; n];
        for i in (0..n).rev() {
            digits[i] = remaining % state.max_num_parts;
            remaining /= state.max_num_parts;
        }
        for (value, digit) in state.elements.iter().zip(digits) {
            let label = digit as usize;
            while parts.len() <= label {
                parts.push(Vec::new());
            }
            parts[label].push(value.clone());
        }
        Literal::AbstractLiteral(AbstractLiteral::Partition(parts))
    }
    fn compactness(state: &State<DomainPtr>) -> usize {
        state.total_size as usize
    }
);

/// Every element only ever uses an *active* part number: unrolled over every position (a packed
/// digit is a *static*-index accessor -- `SafeIndex`-ing it with a *quantified* variable the way
/// `PartitionOccurrence` does for its matrix would need a variable-place-value table lookup plus a
/// variable-divisor `Div`/`Mod`, which `SequencePacked`'s own surjective constraint deliberately
/// avoids for the same reason).
fn bound_which_part_expr(part_numbers: &[Expression], num_parts_ref: &Expression) -> Expression {
    let checks: Vec<Expression> = part_numbers
        .iter()
        .map(|part_number| {
            Expression::Leq(
                Metadata::new(),
                Moo::new(part_number.clone()),
                Moo::new(num_parts_ref.clone()),
            )
        })
        .collect();
    Expression::And(Metadata::new(), Moo::new(into_matrix_expr!(checks)))
}

/// Every active part number is actually used by some element: `forAll k : int(1..maxNumParts), k
/// <= numPartsVar . or([ digit_i = k | i ])`, the "exists" unrolled statically over every position
/// for the same reason as [`bound_which_part_expr`].
fn parts_nonempty_expr(
    part_numbers: &[Expression],
    part_index_domain: &DomainPtr,
    num_parts_ref: &Expression,
) -> Expression {
    crate::types::partition::common::forall_domain(
        part_index_domain,
        "k",
        |k| {
            Some(Expression::Leq(
                Metadata::new(),
                Moo::new(k.clone()),
                Moo::new(num_parts_ref.clone()),
            ))
        },
        |k| {
            let hits: Vec<Expression> = part_numbers
                .iter()
                .map(|part_number| eq(part_number, k))
                .collect();
            Expression::Or(Metadata::new(), Moo::new(into_matrix_expr!(hits)))
        },
    )
}

/// Defines every part number's size, active or not: `forAll k : int(1..maxNumParts) .
/// partSizesVar[k] = sum([ toInt(digit_i = k) | i ])`, the sum's terms unrolled statically over
/// every position for the same reason as [`bound_which_part_expr`].
fn part_sizes_channelling_expr(
    part_numbers: &[Expression],
    part_index_domain: &DomainPtr,
    part_sizes_ref: &Expression,
) -> Expression {
    crate::types::partition::common::forall_domain(
        part_index_domain,
        "k",
        |_k| None,
        |k| {
            let terms: Vec<Expression> = part_numbers
                .iter()
                .map(|part_number| {
                    let matches = eq(part_number, k);
                    essence_expr!(toInt(&matches))
                })
                .collect();
            let sum_expr = Expression::Sum(Metadata::new(), Moo::new(into_matrix_expr!(terms)));
            eq(&index1(part_sizes_ref, k), &sum_expr)
        },
    )
}

/// Breaks the part-labelling symmetry, fully statically unrolled (both the position being checked
/// and the earlier positions it may match are compile-time indices, so no comprehension is needed
/// at all): position `0` always gets part `1`; for `i > 0`, `digit_i > 0` implies some earlier
/// position `j < i` already used `digit_i - 1`.
fn symmetry_breaking_expr(part_numbers: &[Expression]) -> Expression {
    let mut checks = Vec::with_capacity(part_numbers.len());
    for (i, part_i) in part_numbers.iter().enumerate() {
        if i == 0 {
            checks.push(eq(part_i, &int_literal(1)));
            continue;
        }
        let part_i_owned = part_i.clone();
        let part_i_minus_1 = essence_expr!(&part_i_owned - 1);
        let earlier: Vec<Expression> = part_numbers[..i]
            .iter()
            .map(|part_j| eq(part_j, &part_i_minus_1))
            .collect();
        let exists = Expression::Or(Metadata::new(), Moo::new(into_matrix_expr!(earlier)));
        checks.push(implies(gt(part_i, &int_literal(1)), exists));
    }
    Expression::And(Metadata::new(), Moo::new(into_matrix_expr!(checks)))
}
