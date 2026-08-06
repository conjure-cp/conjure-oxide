//! A dense channelling for partitions over a matrix-indexable (`int`) inner domain, adapted from
//! Conjure's `Representations/Partition/Occurrence.hs`.
//!
//! Every inner-domain element gets a "which part is it in" number (`WhichPart`), alongside the
//! active part count (`NumParts`) and each part's size (`PartSizes`). This differs from Conjure's
//! own encoding in one respect: Conjure adds a fourth aux array (`FirstIndex`) purely to break the
//! part-numbering symmetry (every permutation of part *labels* describes the same partition).
//! Here, the same symmetry is broken without an extra array, directly on `WhichPart`: part numbers
//! must be introduced in increasing order as the domain is scanned (`whichPart[i] > 1` implies some
//! earlier element already used `whichPart[i] - 1`) -- this is a standard "first-occurrence"
//! value-symmetry-breaking constraint, and it has the side effect of making Conjure's separate
//! `noGaps` check redundant too (a label can never be skipped if every label above 1 requires an
//! earlier label to already be in use).

use crate::shared::representation_prelude::*;
use crate::types::partition::common::{
    eq, forall_domain, gt, implies, index1, int_literal, leq, lt, minus1, part_size_cons_expr,
    quantified_ref, regular_expr, resolve_partition_size_attrs,
};
use conjure_cp::ast::ac_operators::ACOperatorKind;
use conjure_cp::ast::comprehension::ComprehensionBuilder;
use conjure_cp::ast::{Domain, GroundDomain, Moo, Range, Reference, SymbolTablePtr};

register_representation!(
    PartitionOccurrence("occurrence")
    struct State<T> {
        /// Number of active parts.
        pub num_parts_decl: T,
        /// For each inner-domain element, the (one-based) number of the part it belongs to.
        pub which_part_decl: T,
        /// For each possible part number, how many elements are in it (`0` for an inactive part
        /// number, i.e. one greater than `num_parts_decl`).
        pub part_sizes_decl: T,
        /// The partition's inner (element) domain.
        pub inner_domain: DomainPtr,
        /// `int(1..maxNumParts)`, the domain of a part number -- shared by `which_part_decl`'s
        /// value domain, `part_sizes_decl`'s index domain, and every structural constraint that
        /// quantifies over "all possible part numbers".
        pub part_index_domain: DomainPtr,
        /// The largest number of parts this partition could ever have.
        pub max_num_parts: i32,
        /// The (already bounded, and `regular`-inferred where applicable) part-size attribute,
        /// used to build the per-active-part size constraint.
        pub part_len: Range<i32>,
        /// Whether every part must have equal cardinality.
        pub is_regular: bool,
        /// Whether `partSize` already fixes an exact part size, in which case `regular` is
        /// automatically implied and stating it again would be redundant.
        pub fixed_part_size: bool
    }
    fn init(dom: DomainPtr) -> Result<State<DomainPtr>, ReprInitError> {
        let domain_err = |msg: &str| ReprInitError::UnsupportedDomain(
            dom.clone(),
            PartitionOccurrence::NAME,
            String::from(msg),
        );
        let Some(GroundDomain::Partition(attr, inner)) = dom.as_ground() else {
            return Err(domain_err("expected a ground partition domain"));
        };
        if !can_index_matrix(inner) {
            return Err(domain_err(
                "occurrence representation requires a matrix-indexable (int) inner domain",
            ));
        }
        let inner_domain: DomainPtr = inner.clone().into();
        let max_size = inner_domain
            .length_signed()
            .map_err(|_| domain_err("inner domain must have a known finite size"))?;
        let (num_parts, part_len, fixed_part_size) = resolve_partition_size_attrs(attr, max_size);
        let max_num_parts = num_parts.high().copied().unwrap_or(max_size).max(1);
        let max_part_size = part_len.high().copied().unwrap_or(max_size).max(0);

        let part_index_domain = Domain::int(vec![Range::new(Some(1), Some(max_num_parts))]);
        let num_parts_decl = Domain::int(vec![num_parts]);
        let which_part_decl = Domain::matrix(part_index_domain.clone(), vec![inner_domain.clone()]);
        let part_sizes_decl = Domain::matrix(
            Domain::int(vec![Range::new(Some(0), Some(max_part_size))]),
            vec![part_index_domain.clone()],
        );

        Ok(State {
            num_parts_decl,
            which_part_decl,
            part_sizes_decl,
            inner_domain,
            part_index_domain,
            max_num_parts,
            part_len,
            is_regular: attr.is_regular,
            fixed_part_size,
        })
    }
    fn structural(state: &State<DeclarationPtr>) -> Vec<Expression> {
        let which_part_ref: Expression = Reference::new(state.which_part_decl.clone()).into();
        let part_sizes_ref: Expression = Reference::new(state.part_sizes_decl.clone()).into();
        let num_parts_ref: Expression = Reference::new(state.num_parts_decl.clone()).into();

        let mut constraints = vec![
            bound_which_part_expr(&state.inner_domain, &which_part_ref, &num_parts_ref),
            parts_nonempty_expr(
                &state.part_index_domain,
                &state.inner_domain,
                &which_part_ref,
                &num_parts_ref,
            ),
            part_sizes_channelling_expr(
                &state.part_index_domain,
                &state.inner_domain,
                &which_part_ref,
                &part_sizes_ref,
            ),
            part_size_cons_expr(
                &state.part_index_domain,
                &part_sizes_ref,
                &num_parts_ref,
                &state.part_len,
            ),
            symmetry_breaking_expr(&state.inner_domain, &which_part_ref),
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
        let all_values: Vec<Literal> = state
            .inner_domain
            .values()
            .map_err(|e| ReprDownError::BadValue(
                AbstractLiteral::Partition(parts.clone()).into(),
                format!("could not enumerate the inner domain: {e}"),
            ))?
            .collect();

        // Canonical (first-occurrence) labelling: scan the domain's values in order, handing out
        // the next free label the first time a new part is encountered -- this must agree with
        // `symmetry_breaking_expr`'s own ordering, or a valid partition literal could end up
        // failing the very constraint meant to allow it.
        let mut part_label: Vec<Option<i32>> = vec![None; parts.len()];
        let mut which_part_vals: Vec<Literal> = Vec::with_capacity(all_values.len());
        let mut next_label = 1i32;
        for value in &all_values {
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
            which_part_vals.push(Literal::Int(label));
        }
        let num_parts_val = next_label - 1;

        let mut part_sizes_vals = vec![0i32; state.max_num_parts as usize];
        for (owner, label) in part_label.iter().enumerate() {
            if let Some(label) = label {
                part_sizes_vals[(*label - 1) as usize] = parts[owner].len() as i32;
            }
        }

        let Some(inner_ground) = state.inner_domain.as_ground().cloned() else {
            return Err(ReprDownError::BadValue(
                AbstractLiteral::Partition(parts).into(),
                String::from("inner domain is not ground"),
            ));
        };
        let which_part_decl = Literal::AbstractLiteral(AbstractLiteral::Matrix(
            which_part_vals,
            Moo::new(inner_ground),
        ));
        let part_sizes_decl = Literal::AbstractLiteral(AbstractLiteral::Matrix(
            part_sizes_vals.into_iter().map(Literal::Int).collect(),
            Moo::new(GroundDomain::Int(vec![Range::new(Some(1), Some(state.max_num_parts))])),
        ));

        Ok(State {
            num_parts_decl: Literal::Int(num_parts_val),
            which_part_decl,
            part_sizes_decl,
            inner_domain: state.inner_domain.clone(),
            part_index_domain: state.part_index_domain.clone(),
            max_num_parts: state.max_num_parts,
            part_len: state.part_len.clone(),
            is_regular: state.is_regular,
            fixed_part_size: state.fixed_part_size,
        })
    }
    fn up(state: State<Literal>) -> Literal {
        let Literal::Int(num_parts_val) = state.num_parts_decl else {
            bug!("expected a partition-occurrence NumParts value to be an int, got {}", state.num_parts_decl)
        };
        let Literal::AbstractLiteral(AbstractLiteral::Matrix(which_part_vals, _)) = state.which_part_decl else {
            bug!("expected a partition-occurrence WhichPart value to be a matrix, got {}", state.which_part_decl)
        };
        let all_values: Vec<Literal> = state
            .inner_domain
            .values()
            .unwrap_or_else(|e| bug!("could not enumerate the inner domain: {e}"))
            .collect();

        let mut parts: Vec<Vec<Literal>> = vec![Vec::new(); num_parts_val.max(0) as usize];
        for (value, label) in all_values.into_iter().zip(which_part_vals) {
            let Literal::Int(label) = label else {
                bug!("expected a partition-occurrence WhichPart cell to be an int, got {label}")
            };
            if label >= 1 && label <= num_parts_val {
                parts[(label - 1) as usize].push(value);
            }
        }
        Literal::AbstractLiteral(AbstractLiteral::Partition(parts))
    }
);

/// Only `int` inner domains are supported here, unlike Conjure's broader `domainCanIndexMatrix`
/// (which also allows `bool`/enum): the whole point of this representation is a compact encoding
/// for a sizeable index domain, and a `bool`-indexed partition (at most 2 elements) is already
/// handled perfectly well, and more simply, by `PartitionAsSet`.
fn can_index_matrix(dom: &GroundDomain) -> bool {
    matches!(dom, GroundDomain::Int(_))
}

/// `forAll <outer> : &outer_domain [, outer_guard] . outer_body(&outer, <inner-kind> <inner> :
/// &inner_domain [, inner_guard] . inner_body(&outer, &inner))`.
///
/// The inner comprehension's generator is built as a **domain-based** `Generator` sharing the
/// outer builder's own symbol table as its parent scope, rather than as an `ExpressionGenerator`:
/// `Comprehension` is a Uniplate leaf in this codebase, so a nested comprehension's own qualifiers
/// are never independently visited by the generic rewrite engine while nested inside an outer
/// comprehension's return expression -- building it this way sidesteps that entirely, mirroring
/// `PartitionAsSet`'s own `exactly_once_expr`.
#[allow(clippy::too_many_arguments)]
fn forall_nested(
    outer_domain: &DomainPtr,
    outer_name: &str,
    outer_guard: impl FnOnce(&Expression) -> Option<Expression>,
    inner_domain: &DomainPtr,
    inner_name: &str,
    inner_kind: ACOperatorKind,
    inner_guard: impl FnOnce(&Expression, &Expression) -> Option<Expression>,
    inner_body: impl FnOnce(&Expression, &Expression) -> Expression,
    outer_body: impl FnOnce(&Expression, Expression) -> Expression,
) -> Expression {
    let outer_var = Name::user(outer_name);
    let mut outer_builder = ComprehensionBuilder::new(SymbolTablePtr::new()).generator(
        DeclarationPtr::new_find(outer_var.clone(), outer_domain.clone()),
    );
    let outer_symbols = outer_builder.generator_symboltable();
    let outer_ref = quantified_ref(&outer_symbols, &outer_var);
    if let Some(g) = outer_guard(&outer_ref) {
        outer_builder = outer_builder.guard(g);
    }

    let inner_var = Name::user(inner_name);
    let mut inner_builder = ComprehensionBuilder::new(outer_symbols).generator(
        DeclarationPtr::new_find(inner_var.clone(), inner_domain.clone()),
    );
    let inner_symbols = inner_builder.return_expr_symboltable();
    let inner_ref = quantified_ref(&inner_symbols, &inner_var);
    if let Some(g) = inner_guard(&outer_ref, &inner_ref) {
        inner_builder = inner_builder.guard(g);
    }
    let inner_return = inner_body(&outer_ref, &inner_ref);
    let mut inner_comprehension = inner_builder.with_return_value(inner_return);
    inner_comprehension.skip_operator = Some(inner_kind);
    let inner_wrapped = Expression::Comprehension(Metadata::new(), Moo::new(inner_comprehension));
    let inner_expr = match inner_kind {
        ACOperatorKind::And => Expression::And(Metadata::new(), Moo::new(inner_wrapped)),
        ACOperatorKind::Or => Expression::Or(Metadata::new(), Moo::new(inner_wrapped)),
        ACOperatorKind::Sum => Expression::Sum(Metadata::new(), Moo::new(inner_wrapped)),
        ACOperatorKind::Product => Expression::Product(Metadata::new(), Moo::new(inner_wrapped)),
        // Every caller in this file passes And/Or/Sum only.
        ACOperatorKind::Min | ACOperatorKind::Max => unreachable!(),
    };

    let outer_return = outer_body(&outer_ref, inner_expr);
    let mut outer_comprehension = outer_builder.with_return_value(outer_return);
    outer_comprehension.skip_operator = Some(ACOperatorKind::And);
    let outer_wrapped = Expression::Comprehension(Metadata::new(), Moo::new(outer_comprehension));
    Expression::And(Metadata::new(), Moo::new(outer_wrapped))
}

/// Every element only ever uses an *active* part number: `forAll i : innerDomain . whichPart[i] <=
/// numPartsVar`.
fn bound_which_part_expr(
    inner_domain: &DomainPtr,
    which_part_ref: &Expression,
    num_parts_ref: &Expression,
) -> Expression {
    forall_domain(
        inner_domain,
        "i",
        |_| None,
        |i| leq(&index1(which_part_ref, i), num_parts_ref),
    )
}

/// Every active part number is actually used by some element: `forAll k : int(1..maxNumParts), k
/// <= numPartsVar . exists j : innerDomain . whichPart[j] = k`. Together with
/// [`symmetry_breaking_expr`] (which prevents a used label from ever being introduced out of
/// order), this pins `numPartsVar` to exactly the number of distinct labels in use -- mirrors
/// Conjure's `noGaps`, without that rule's `int(3..maxNumParts)` special-casing (not needed here).
fn parts_nonempty_expr(
    part_index_domain: &DomainPtr,
    inner_domain: &DomainPtr,
    which_part_ref: &Expression,
    num_parts_ref: &Expression,
) -> Expression {
    forall_nested(
        part_index_domain,
        "k",
        |k| Some(leq(k, num_parts_ref)),
        inner_domain,
        "j",
        ACOperatorKind::Or,
        |_k, _j| None,
        |k, j| eq(&index1(which_part_ref, j), k),
        |_k, exists_expr| exists_expr,
    )
}

/// Defines every part number's size, active or not: `forAll k : int(1..maxNumParts) .
/// partSizesVar[k] = sum([ 1 | j : innerDomain, whichPart[j] = k ])`. Mirrors Conjure's
/// `partSizesChannelling`; an inactive part number's size comes out `0` automatically here since
/// [`bound_which_part_expr`] already guarantees no element ever uses one.
fn part_sizes_channelling_expr(
    part_index_domain: &DomainPtr,
    inner_domain: &DomainPtr,
    which_part_ref: &Expression,
    part_sizes_ref: &Expression,
) -> Expression {
    forall_nested(
        part_index_domain,
        "k",
        |_k| None,
        inner_domain,
        "j",
        ACOperatorKind::Sum,
        |k, j| Some(eq(&index1(which_part_ref, j), k)),
        |_k, _j| int_literal(1),
        |k, sum_expr| eq(&index1(part_sizes_ref, k), &sum_expr),
    )
}

/// Breaks the part-labelling symmetry (every permutation of part numbers describes the same
/// partition) without a separate `FirstIndex` array: part numbers must be introduced in increasing
/// order as the domain is scanned. `forAll i : innerDomain . whichPart[i] > 1 -> exists j :
/// innerDomain, j < i . whichPart[j] = whichPart[i] - 1`.
fn symmetry_breaking_expr(inner_domain: &DomainPtr, which_part_ref: &Expression) -> Expression {
    forall_nested(
        inner_domain,
        "i",
        |_i| None,
        inner_domain,
        "j",
        ACOperatorKind::Or,
        |i, j| Some(lt(j, i)),
        |i, j| {
            eq(
                &index1(which_part_ref, j),
                &minus1(&index1(which_part_ref, i)),
            )
        },
        |i, exists_expr| implies(gt(&index1(which_part_ref, i), &int_literal(1)), exists_expr),
    )
}
