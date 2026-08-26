//! Indexing and slicing a matrix literal.
//!
//! Slicing a *variable* is the business of whichever representation that variable was given --
//! [`MatrixComponents`](super::MatrixComponents) has a rule for it. A matrix *literal* has no
//! representation to consult, and one turns up whenever a layout has already been decoded back
//! into an ordinary matrix expression, as [`MatrixPacked`](super::MatrixPacked) does. Without this,
//! `allDiff(a[..,1])` over a packed matrix reaches the backend with the slice still in it, and
//! `m[m[2]]` over one indexed from two reaches it counting from the wrong place.

use conjure_cp::ast::{
    AbstractLiteral, DomainPtr, Expression as Expr, Literal, Metadata, Moo, SymbolTable,
    eval_constant,
};
use conjure_cp::rule_engine::{
    ApplicationError::RuleNotApplicable, ApplicationResult, RuleEffect, register_rule,
};
use conjure_cp::{essence_expr, into_matrix_expr};

/// Resolve a slice of a matrix literal into the sub-matrix it names.
#[register_rule("Base", 5000, [SafeSlice])]
fn slice_matrix_literal(expr: &Expr, _: &SymbolTable) -> ApplicationResult {
    let Expr::SafeSlice(_, subject, dimensions) = expr else {
        return Err(RuleNotApplicable);
    };
    // References are sliced by their representation's own rules, which know where the entries
    // actually live.
    if !matches!(
        subject.as_ref(),
        Expr::AbstractLiteral(_, AbstractLiteral::Matrix(..))
    ) {
        return Err(RuleNotApplicable);
    }

    slice_literal(subject.as_ref(), dimensions)
        .map(RuleEffect::pure)
        .ok_or(RuleNotApplicable)
}

/// Walk the dimensions, taking one entry where the slice names an index and keeping every entry
/// where it does not.
fn slice_literal(subject: &Expr, dimensions: &[Option<Expr>]) -> Option<Expr> {
    let Some((first, rest)) = dimensions.split_first() else {
        return Some(subject.clone());
    };

    let Expr::AbstractLiteral(_, AbstractLiteral::Matrix(elements, index_domain)) = subject else {
        return None;
    };

    match first {
        // A named index picks one entry out of this dimension, and drops it from the result.
        Some(index) => {
            let index = eval_constant(index)?;
            let inner = element_at(elements, index_domain, &index)?;
            slice_literal(inner, rest)
        }
        // An open dimension keeps every entry, and each is sliced by what remains.
        None => {
            let sliced = elements
                .iter()
                .map(|element| slice_literal(element, rest))
                .collect::<Option<Vec<_>>>()?;
            Some(Expr::AbstractLiteral(
                Metadata::new(),
                AbstractLiteral::Matrix(sliced, index_domain.clone()),
            ))
        }
    }
}

/// The entry an index names, whether the index domain says which values it holds or not.
///
/// A matrix written as a plain list carries the implied index domain `int(1..)`, which cannot be
/// enumerated -- its entries are simply at one-based positions. A matrix that kept a declared index
/// domain may start anywhere, so there the position is where the value sits in that domain.
fn element_at<'a>(
    elements: &'a [Expr],
    index_domain: &DomainPtr,
    index: &Literal,
) -> Option<&'a Expr> {
    if let Ok(ground) = index_domain.resolve()
        && let Ok(values) = ground.values()
        && let Some(position) = values
            .enumerate()
            .find(|(_, value)| value.essence_cmp(index).is_eq())
            .map(|(position, _)| position)
    {
        return elements.get(position);
    }

    let Literal::Int(value) = index else {
        return None;
    };
    elements.get(usize::try_from(value - 1).ok()?)
}

/// Shift a variable index into a matrix literal whose index domain does not start at one.
///
/// Backends index a literal by position: Minion's `element` and Z3's arrays both count from where
/// the list starts. A variable index into `matrix indexed by [int(2..4)]` therefore has to be
/// moved to a one-based position first. A constant index needs none of this -- it is resolved
/// outright by the rules above.
#[register_rule("Base", 5000, [SafeIndex])]
fn shift_matrix_literal_index(expr: &Expr, _: &SymbolTable) -> ApplicationResult {
    let Expr::SafeIndex(_, subject, indices) = expr else {
        return Err(RuleNotApplicable);
    };
    if indices.is_empty() || indices.iter().all(|index| eval_constant(index).is_some()) {
        return Err(RuleNotApplicable);
    }

    // Each dimension's index domain lives on the literal at that depth.
    let mut lows = Vec::with_capacity(indices.len());
    let mut cursor = subject.as_ref();
    for _ in 0..indices.len() {
        let Expr::AbstractLiteral(_, AbstractLiteral::Matrix(elements, index_domain)) = cursor
        else {
            return Err(RuleNotApplicable);
        };
        lows.push(contiguous_low(index_domain).ok_or(RuleNotApplicable)?);
        cursor = elements.first().ok_or(RuleNotApplicable)?;
    }

    if lows.iter().all(|low| *low == 1) {
        return Err(RuleNotApplicable);
    }

    let shifted = indices
        .iter()
        .zip(lows.iter())
        .map(|(index, low)| {
            if *low == 1 {
                index.clone()
            } else {
                let offset = 1 - low;
                essence_expr!(&index + &offset)
            }
        })
        .collect();

    Ok(RuleEffect::pure(Expr::SafeIndex(
        Metadata::new(),
        Moo::new(to_implied_indices(subject.as_ref(), indices.len())),
        shifted,
    )))
}

/// The first value of an index domain, when its values run consecutively from there.
///
/// A gappy domain has no shift that turns a value into a position, so those are left alone.
fn contiguous_low(index_domain: &DomainPtr) -> Option<i32> {
    let ground = index_domain.resolve().ok()?;
    let mut values = ground.values().ok()?.map(|value| match value {
        Literal::Int(value) => Some(value),
        _ => None,
    });

    let first = values.next()??;
    let mut previous = first;
    for value in values {
        let value = value?;
        if value != previous + 1 {
            return None;
        }
        previous = value;
    }
    Some(first)
}

/// Restate a matrix literal's outer `depth` dimensions as plain one-based lists.
fn to_implied_indices(subject: &Expr, depth: usize) -> Expr {
    if depth == 0 {
        return subject.clone();
    }
    let Expr::AbstractLiteral(_, AbstractLiteral::Matrix(elements, _)) = subject else {
        return subject.clone();
    };
    let elements = elements
        .iter()
        .map(|element| to_implied_indices(element, depth - 1))
        .collect::<Vec<_>>();
    into_matrix_expr!(elements)
}
