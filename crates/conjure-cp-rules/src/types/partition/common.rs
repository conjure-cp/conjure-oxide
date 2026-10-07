//! Shared helpers used by every partition representation.

use conjure_cp::ast::ac_operators::ACOperatorKind;
use conjure_cp::ast::comprehension::ComprehensionBuilder;
use conjure_cp::ast::{
    Atom, DeclarationPtr, DomainPtr, Expression, Literal, Metadata, Moo, Name, PartitionAttr,
    Range, Reference, SymbolTablePtr,
};
use conjure_cp::bug;

/// Narrows a size-shaped range attribute to be bounded above by `max`, treating an absent lower
/// bound as `0` and an absent upper bound as `max`.
pub(crate) fn bound_size(range: &Range<i32>, max: i32) -> Range<i32> {
    let lo = range.low().copied().unwrap_or(0).max(0);
    let hi = range.high().copied().map(|h| h.min(max)).unwrap_or(max);
    Range::new(Some(lo), Some(hi))
}

/// Resolves a partition's `numParts`/`partSize` attributes into concrete, bounded ranges, and
/// reports whether the part size ends up fixed. Shared by every partition representation so they
/// agree on exactly the same notion of "how big can this get" and "is `regular` redundant here".
///
/// If the partition is `regular` and either `numParts` or `partSize` is already pinned to a single
/// value, the other is forced (`partSize = |innerDomain| / numParts`, or vice versa) -- a regular
/// partition's parts are all the same size, so pinning one of (count, size) with a known total
/// forces the other. This is treated exactly as if the user had written the forced value
/// explicitly, which lets `regular`'s own pairwise "all parts equal size" check be skipped
/// entirely wherever it would otherwise be redundant. When the total doesn't divide evenly no
/// regular partition exists; the attributes are left as given so the model comes out UNSAT via the
/// normal size constraints rather than special-casing infeasibility here.
///
/// Both `numParts` and `partSize` are also bounded above by the inner domain's own size
/// (`max_size`), falling back to that bound whenever the attribute itself leaves size unbounded --
/// mirrors Conjure's own `getMaxNumParts`/`getMaxPartSizes` (`Representations/Partition/
/// Occurrence.hs`), which fall back to `domainSizeOf` the same way.
pub(crate) fn resolve_partition_size_attrs(
    attr: &PartitionAttr,
    max_size: i32,
) -> (Range<i32>, Range<i32>, bool) {
    let inferred_part_len = if attr.is_regular {
        match (
            attr.num_parts.low(),
            attr.num_parts.high(),
            attr.part_len.low(),
            attr.part_len.high(),
        ) {
            (Some(n), Some(n2), _, _) if n == n2 && *n != 0 && max_size % n == 0 => {
                Some(Range::Single(max_size / n))
            }
            (_, _, Some(s), Some(s2)) if s == s2 => None,
            _ => None,
        }
    } else {
        None
    };
    let inferred_num_parts = if attr.is_regular {
        match (
            attr.part_len.low(),
            attr.part_len.high(),
            attr.num_parts.low(),
            attr.num_parts.high(),
        ) {
            (Some(s), Some(s2), np_lo, np_hi)
                if s == s2
                    && *s != 0
                    && max_size % s == 0
                    && !(np_lo.is_some() && np_hi.is_some() && np_lo == np_hi) =>
            {
                Some(Range::Single(max_size / s))
            }
            _ => None,
        }
    } else {
        None
    };
    let num_parts = bound_size(
        &inferred_num_parts.unwrap_or_else(|| attr.num_parts.clone()),
        max_size,
    );
    let part_len = bound_size(
        &inferred_part_len.unwrap_or_else(|| attr.part_len.clone()),
        max_size,
    );
    let fixed_part_size = matches!(part_len, Range::Single(_));
    (num_parts, part_len, fixed_part_size)
}

pub(crate) fn eq(a: &Expression, b: &Expression) -> Expression {
    Expression::Eq(Metadata::new(), Moo::new(a.clone()), Moo::new(b.clone()))
}

pub(crate) fn leq(a: &Expression, b: &Expression) -> Expression {
    Expression::Leq(Metadata::new(), Moo::new(a.clone()), Moo::new(b.clone()))
}

pub(crate) fn lt(a: &Expression, b: &Expression) -> Expression {
    Expression::Lt(Metadata::new(), Moo::new(a.clone()), Moo::new(b.clone()))
}

pub(crate) fn gt(a: &Expression, b: &Expression) -> Expression {
    Expression::Gt(Metadata::new(), Moo::new(a.clone()), Moo::new(b.clone()))
}

pub(crate) fn implies(a: Expression, b: Expression) -> Expression {
    Expression::Imply(Metadata::new(), Moo::new(a), Moo::new(b))
}

pub(crate) fn minus1(e: &Expression) -> Expression {
    Expression::Minus(
        Metadata::new(),
        Moo::new(e.clone()),
        Moo::new(int_literal(1)),
    )
}

pub(crate) fn int_literal(n: i32) -> Expression {
    Expression::Atomic(Metadata::new(), Atom::Literal(Literal::Int(n)))
}

pub(crate) fn index1(matrix_ref: &Expression, idx: &Expression) -> Expression {
    Expression::SafeIndex(
        Metadata::new(),
        Moo::new(matrix_ref.clone()),
        vec![idx.clone()],
    )
}

/// Builds the constraint checking that `expr` satisfies a size-shaped range attribute.
pub(crate) fn range_body(range: &Range<i32>, expr: &Expression) -> Expression {
    match range {
        Range::Unbounded => Expression::Atomic(Metadata::new(), Atom::Literal(Literal::Bool(true))),
        Range::Single(n) => eq(expr, &int_literal(*n)),
        Range::UnboundedR(min) => Expression::Geq(
            Metadata::new(),
            Moo::new(expr.clone()),
            Moo::new(int_literal(*min)),
        ),
        Range::UnboundedL(max) => Expression::Leq(
            Metadata::new(),
            Moo::new(expr.clone()),
            Moo::new(int_literal(*max)),
        ),
        Range::Bounded(min, max) => {
            let lo = Expression::Geq(
                Metadata::new(),
                Moo::new(expr.clone()),
                Moo::new(int_literal(*min)),
            );
            let hi = Expression::Leq(
                Metadata::new(),
                Moo::new(expr.clone()),
                Moo::new(int_literal(*max)),
            );
            Expression::And(Metadata::new(), Moo::new(conjure_cp::matrix_expr![lo, hi]))
        }
    }
}

/// Looks up a just-inserted quantified variable's declaration by name.
pub(crate) fn quantified_ref(symbols: &SymbolTablePtr, name: &Name) -> Expression {
    let decl = symbols
        .read()
        .lookup(name)
        .unwrap_or_else(|| bug!("expected a quantified variable {name} to be in scope"));
    Reference::new(decl).into()
}

/// `forAll <name> : &domain [, guard(&ref)] . body(&ref)`.
pub(crate) fn forall_domain(
    domain: &DomainPtr,
    name: &str,
    guard: impl FnOnce(&Expression) -> Option<Expression>,
    body: impl FnOnce(&Expression) -> Expression,
) -> Expression {
    let var_name = Name::user(name);
    let mut builder = ComprehensionBuilder::new(SymbolTablePtr::new())
        .generator(DeclarationPtr::new_find(var_name.clone(), domain.clone()));
    let symbols = builder.generator_symboltable();
    let var_ref = quantified_ref(&symbols, &var_name);
    if let Some(g) = guard(&var_ref) {
        builder = builder.guard(g);
    }
    let return_expr = body(&var_ref);
    let mut comprehension = builder.with_return_value(return_expr);
    comprehension.skip_operator = Some(ACOperatorKind::And);
    let wrapped = Expression::Comprehension(Metadata::new(), Moo::new(comprehension));
    Expression::And(Metadata::new(), Moo::new(wrapped))
}

/// Every *active* part satisfies the `partSize` attribute: `forAll k : int(1..maxNumParts), k <=
/// numPartsVar . <part_len's range check on partSizesVar[k]>`. Mirrors Conjure's `partSizeCons`;
/// inactive parts are left unconstrained here, on the assumption every representation already
/// pins them to size `0` some other way.
pub(crate) fn part_size_cons_expr(
    part_index_domain: &DomainPtr,
    part_sizes_ref: &Expression,
    num_parts_ref: &Expression,
    part_len: &Range<i32>,
) -> Expression {
    forall_domain(
        part_index_domain,
        "k",
        |k| Some(leq(k, num_parts_ref)),
        |k| range_body(part_len, &index1(part_sizes_ref, k)),
    )
}

/// Every part has the same cardinality: `forAll k : int(2..maxNumParts), k <= numPartsVar .
/// partSizesVar[k-1] = partSizesVar[k]`. Mirrors Conjure's `regular` (only emitted when the part
/// size isn't already fixed by `partSize`).
pub(crate) fn regular_expr(
    regular_index_domain: &DomainPtr,
    part_sizes_ref: &Expression,
    num_parts_ref: &Expression,
) -> Expression {
    forall_domain(
        regular_index_domain,
        "k",
        |k| Some(leq(k, num_parts_ref)),
        |k| {
            eq(
                &index1(part_sizes_ref, &minus1(k)),
                &index1(part_sizes_ref, k),
            )
        },
    )
}
