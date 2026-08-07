use super::TuplePacked;
use crate::guard;
use crate::shared::utils::{
    as_cmp_or_lex_op, as_eq_or_neq, collect_cmp_exprs, collect_eq_or_neq, eq_or_neq,
    tuple_expr_entries,
};
use crate::types::tuple::TupleComponents;
use conjure_cp::ast::{
    AbstractLiteral, Atom, Expression as Expr, Literal, Metadata, Moo, Reference, SymbolTable,
    eval_constant,
};
use conjure_cp::bug_assert;
use conjure_cp::representation::ReprRule;
use conjure_cp::rule_engine::ApplicationError::{self, RuleNotApplicable};
use conjure_cp::rule_engine::{
    ApplicationResult, RuleEffect as Reduction, register_rule, register_rule_set,
};
use conjure_cp::{essence_expr, into_matrix_expr};
use parking_lot::MappedRwLockReadGuard;

fn comparison_reference(expr: &Expr) -> Option<Reference> {
    if let Expr::Atomic(_, Atom::Reference(reference)) = expr {
        return Some(reference.clone());
    }
    let values = expr.unwrap_list()?;
    let [Expr::Atomic(_, Atom::Reference(reference))] = values.as_slice() else {
        return None;
    };
    Some(reference.clone())
}

// Packed tuples are backend-neutral, so make their lowering rules available
// for every solver family.
register_rule_set!("ReprTuplePacked", ("Base"), |_| true);

/// Equality of packed tuple variables.
///
/// Matching layouts can be compared directly. Different layouts are decoded
/// and compared element-wise because the same packed value can represent
/// different tuples in each layout.
/// ```plain
/// x = y  (both TuplePacked)  ~>  x_packed = y_packed
/// ```
#[register_rule("ReprTuplePacked", 9700, [Eq, Neq])]
fn tuple_packed_var_eq_var(expr: &Expr, _: &SymbolTable) -> ApplicationResult {
    let (lhs, rhs, neq) = as_eq_or_neq(expr)?;

    guard!(
        let Expr::Atomic(_, Atom::Reference(re))  = lhs &&
        let Some(lp) = re.get_repr_as::<TuplePacked>()  &&
        let Expr::Atomic(_, Atom::Reference(re2)) = rhs &&
        let Some(rp) = re2.get_repr_as::<TuplePacked>()
        else { return Err(RuleNotApplicable) }
    );

    bug_assert!(
        lp.values.len() == rp.values.len(),
        "equality on tuples with different shapes!"
    );

    let new_expr = if layouts_match(&lp, &rp) {
        eq_or_neq(neq, lp.packed_expr(), rp.packed_expr())
    } else {
        collect_eq_or_neq(
            neq,
            unpack_entries(&lp).into_iter().zip(unpack_entries(&rp)),
        )
    };
    Ok(Reduction::pure(new_expr))
}

/// Equality of packed tuple variable to a tuple literal
/// ```plain
/// x = (1, 2, 3)  (x is TuplePacked)  ~>  x_packed = encode(1,2,3)
/// ```
#[register_rule("ReprTuplePacked", 9700, [Eq, Neq])]
fn tuple_packed_var_eq_lit(expr: &Expr, _: &SymbolTable) -> ApplicationResult {
    let (lhs, rhs, neq) = as_eq_or_neq(expr)?;

    guard!(
        let Expr::Atomic(_, Atom::Reference(re)) = lhs   &&
        let Some(repr) = re.get_repr_as::<TuplePacked>() &&
        let Some(rhs_ents) = tuple_expr_entries(rhs)
        else { return Err(RuleNotApplicable) }
    );

    bug_assert!(
        repr.values.len() == rhs_ents.len(),
        "equality on tuples with different shapes!"
    );

    let packed_val = encode_entries(&repr, &rhs_ents)?;
    let new_expr = eq_or_neq(neq, repr.packed_expr(), packed_val);
    Ok(Reduction::pure(new_expr))
}

/// Comparison of packed tuple variables.
///
/// Matching layouts preserve lexicographic order directly. Different layouts
/// are decoded before applying the tuple comparison.
/// ```plain
/// x > y  (both TuplePacked)  ~>  x_packed > y_packed
/// ```
#[register_rule(
    "ReprTuplePacked",
    9700,
    [Lt, Gt, Leq, Geq, LexLt, LexGt, LexLeq, LexGeq]
)]
fn tuple_packed_var_cmp_var(expr: &Expr, _: &SymbolTable) -> ApplicationResult {
    guard!(
        as_eq_or_neq(expr).is_err() &&
        let Some((lhs, rhs)) = as_cmp_or_lex_op(expr)               &&
        let Some(lhs_re) = comparison_reference(lhs.as_ref())       &&
        let Some(rhs_re) = comparison_reference(rhs.as_ref())       &&
        let Some(lp) = lhs_re.get_repr_as::<TuplePacked>()          &&
        let Some(rp) = rhs_re.get_repr_as::<TuplePacked>()
        else { return Err(RuleNotApplicable) }
    );

    bug_assert!(
        lp.values.len() == rp.values.len(),
        "comparison of tuples with different shapes!"
    );

    let new_expr = if layouts_match(&lp, &rp) {
        packed_cmp(expr, lp.packed_expr(), rp.packed_expr())
    } else {
        collect_cmp_exprs(expr, unpack_entries(&lp), unpack_entries(&rp))
    };
    Ok(Reduction::pure(new_expr))
}

/// Comparison of packed tuple variable to a literal
/// ```plain
/// x > (1,2,3)  (x is TuplePacked)  ~>  x_packed > encode(1,2,3)
/// ```
#[register_rule(
    "ReprTuplePacked",
    9700,
    [Lt, Gt, Leq, Geq, LexLt, LexGt, LexLeq, LexGeq]
)]
fn tuple_packed_var_cmp_lit(expr: &Expr, _: &SymbolTable) -> ApplicationResult {
    guard!(
        as_eq_or_neq(expr).is_err() &&
        let Some((lhs, rhs)) = as_cmp_or_lex_op(expr)               &&
        let Expr::Atomic(_, Atom::Reference(lhs_re)) = lhs.as_ref() &&
        let Some(repr) = lhs_re.get_repr_as::<TuplePacked>()        &&
        let Some(rhs_ents) = tuple_expr_entries(&rhs)
        else { return Err(RuleNotApplicable) }
    );

    let packed_val = encode_entries(&repr, &rhs_ents)?;
    Ok(Reduction::pure(packed_cmp(
        expr,
        repr.packed_expr(),
        packed_val,
    )))
}

/// Indexing into a packed tuple variable
/// ```plain
/// x[i]  (x is TuplePacked)  ~>  (x_packed / stride_i) % size_i + min_i
/// ```
#[register_rule("ReprTuplePacked", 9700, [SafeIndex])]
fn tuple_packed_index_lit(expr: &Expr, _: &SymbolTable) -> ApplicationResult {
    guard!(
        let Expr::SafeIndex(_, subject, indices) = expr       &&
        let Expr::Atomic(_, Atom::Reference(re)) = &**subject &&
        let Some(Expr::Atomic(_, idx)) = indices.first()      &&
        let Atom::Literal(Literal::Int(idx)) = idx            &&
        let Some(repr) = re.get_repr_as::<TuplePacked>()
        else { return Err(RuleNotApplicable) }
    );

    let i = (*idx - 1) as usize;
    bug_assert!(i < repr.values.len(), "tuple indexing is out of bounds");

    let remaining = &indices[1..];
    if remaining.is_empty() {
        Ok(Reduction::pure(unpack_entry(&repr, i)))
    } else if let Some(projected) = project_values(&repr.values[i], remaining) {
        Ok(Reduction::pure(unpack_values(&repr, i, &projected)))
    } else {
        Ok(Reduction::pure(Expr::SafeIndex(
            Metadata::new(),
            unpack_entry(&repr, i).into(),
            Vec::from(remaining),
        )))
    }
}

/// Channeling constraint between TupleComponents and TuplePacked for the same variable.
/// Handles equalities of the form `x#TupleComponents = x#TuplePacked` (or reversed).
/// ```plain
/// x#TupleComponents = x#TuplePacked
/// ~>
/// decoded packed fields = component fields
/// ```
#[register_rule("ReprTuplePacked", 9700, [Eq, Neq])]
fn tuple_channel_atom_packed(expr: &Expr, _: &SymbolTable) -> ApplicationResult {
    let (lhs, rhs, neq) = as_eq_or_neq(expr)?;

    guard!(
        let Expr::Atomic(_, Atom::Reference(re_a)) = lhs          &&
        let Expr::Atomic(_, Atom::Reference(re_b)) = rhs          &&
        let Some((packed, atom)) = as_channeling_pair(re_a, re_b)
        else {
            return Err(RuleNotApplicable);
        }
    );

    let fields = atom.elems.iter().enumerate().map(|(index, declaration)| {
        (
            unpack_entry(&packed, index),
            Expr::from(Reference::new(declaration.clone())),
        )
    });
    Ok(Reduction::pure(collect_eq_or_neq(neq, fields)))
}

/// Build a scalar comparison matching the given (possibly lex) comparison operator.
/// Packed tuples are single integers, so `LexLt` → `Lt`, etc.
fn packed_cmp(op: &Expr, lhs: Expr, rhs: Expr) -> Expr {
    let (lhs, rhs) = (Moo::new(lhs), Moo::new(rhs));
    match op {
        Expr::Lt(..) | Expr::LexLt(..) => Expr::Lt(Metadata::new(), lhs, rhs),
        Expr::Leq(..) | Expr::LexLeq(..) => Expr::Leq(Metadata::new(), lhs, rhs),
        Expr::Gt(..) | Expr::LexGt(..) => Expr::Gt(Metadata::new(), lhs, rhs),
        Expr::Geq(..) | Expr::LexGeq(..) => Expr::Geq(Metadata::new(), lhs, rhs),
        _ => unreachable!("packed_cmp: unexpected operator"),
    }
}

fn layouts_match(lhs: &PackedState<'_>, rhs: &PackedState<'_>) -> bool {
    lhs.values == rhs.values
}

fn unpack_entries(repr: &PackedState<'_>) -> Vec<Expr> {
    (0..repr.values.len())
        .map(|index| unpack_entry(repr, index))
        .collect()
}

fn unpack_entry(repr: &PackedState<'_>, index: usize) -> Expr {
    unpack_values(repr, index, &repr.values[index])
}

fn unpack_values(repr: &PackedState<'_>, index: usize, values: &[Literal]) -> Expr {
    let packed = repr.packed_expr();
    let place = repr.places[index];
    let radix = repr.radices[index];
    let digit = match (place, radix, index) {
        (_, 1, _) => Expr::from(0),
        (1, _, 0) => packed,
        (_, _, 0) => essence_expr!(&packed / &place),
        (1, radix, _) => essence_expr!(&packed % &radix),
        (_, radix, _) => essence_expr!((&packed / &place) % &radix),
    };
    unpack_digit(&digit, values)
}

/// Given a decoded position `digit` (a decision-variable expression, not necessarily constant --
/// this is what makes the compound case below non-trivial) and the ordered list of possible
/// literal values at that position, build an expression for "the value at position `digit`".
///
/// Scalar (bool, or non-contiguous int) values fall back to a plain `SafeIndex` into a literal
/// matrix of those values, dynamically indexed by `digit` -- the Minion backend already supports
/// that (the same shape `FunctionExplicit::values_matrix` uses). A *compound* value (currently:
/// tuple; see the module doc for why record/variant aren't handled here yet) can't take that path:
/// indexing a matrix of compound elements by a non-constant position is not something the backend
/// can turn into a per-field `Element` constraint chain, and it silently produces an unresolved
/// literal handed straight to the solver ("expected a literal but got `AbstractLiteral(...)`).
/// Instead, each candidate tuple's own fields are projected out (all candidates share the same
/// arity, since `symmetry_values` builds them from one domain), and the value is rebuilt as an
/// inline tuple literal expression whose *own* fields are each decoded recursively by this same
/// function -- reusing `digit`, since every candidate is keyed by the same packed position. A
/// field that's itself a contiguous int range (the common case) hits the cheap arithmetic path one
/// level down; a doubly-nested tuple recurses again.
fn unpack_digit(digit: &Expr, values: &[Literal]) -> Expr {
    if let Some(minimum) = contiguous_int_min(values) {
        return match minimum {
            0 => digit.clone(),
            minimum => essence_expr!(&digit + &minimum),
        };
    }
    if let Some(rebuilt) = unpack_compound_digit(digit, values) {
        return rebuilt;
    }
    let values = values.iter().cloned().map(Expr::from).collect::<Vec<_>>();
    Expr::SafeIndex(
        Metadata::new(),
        Moo::new(into_matrix_expr!(values)),
        vec![essence_expr!(&digit + 1)],
    )
}

/// The compound-value case of [`unpack_digit`]: `None` unless every value is a tuple literal of
/// the same arity (guaranteed for a genuine packed field's own value list, but checked directly
/// rather than assumed, since a mismatch here should fall back to the -- still correct, if
/// unreduced until something else eliminates it -- generic path instead of panicking).
fn unpack_compound_digit(digit: &Expr, values: &[Literal]) -> Option<Expr> {
    let Literal::AbstractLiteral(AbstractLiteral::Tuple(first_fields)) = values.first()? else {
        return None;
    };
    let arity = first_fields.len();

    let mut field_values: Vec<Vec<Literal>> = vec![Vec::with_capacity(values.len()); arity];
    for value in values {
        let Literal::AbstractLiteral(AbstractLiteral::Tuple(fields)) = value else {
            return None;
        };
        if fields.len() != arity {
            return None;
        }
        for (slot, field) in field_values.iter_mut().zip(fields) {
            slot.push(field.clone());
        }
    }

    let rebuilt_fields = field_values
        .into_iter()
        .map(|values| unpack_digit(digit, &values))
        .collect();
    Some(Expr::AbstractLiteral(
        Metadata::new(),
        AbstractLiteral::Tuple(rebuilt_fields),
    ))
}

fn project_values(values: &[Literal], indices: &[Expr]) -> Option<Vec<Literal>> {
    values
        .iter()
        .map(|value| {
            indices.iter().try_fold(value.clone(), |value, index| {
                let Literal::Int(index) = eval_constant(index)? else {
                    return None;
                };
                let Literal::AbstractLiteral(conjure_cp::ast::AbstractLiteral::Tuple(fields)) =
                    value
                else {
                    return None;
                };
                usize::try_from(index - 1)
                    .ok()
                    .and_then(|index| fields.get(index).cloned())
            })
        })
        .collect()
}

fn contiguous_int_min(values: &[Literal]) -> Option<i32> {
    let Literal::Int(minimum) = values.first()? else {
        return None;
    };
    values
        .iter()
        .enumerate()
        .all(|(offset, value)| *value == Literal::Int(*minimum + offset as i32))
        .then_some(*minimum)
}

fn encode_entries(repr: &PackedState<'_>, entries: &[Expr]) -> Result<Expr, ApplicationError> {
    let values = entries
        .iter()
        .map(eval_constant)
        .collect::<Option<Vec<_>>>()
        .ok_or(RuleNotApplicable)?;
    repr.encode(&values)
        .map(Expr::from)
        .ok_or(RuleNotApplicable)
}

type PackedState<'a> = MappedRwLockReadGuard<'a, <TuplePacked as ReprRule>::DeclLevel>;
type ComponentsState<'a> = MappedRwLockReadGuard<'a, <TupleComponents as ReprRule>::DeclLevel>;
fn as_channeling_pair<'a>(
    lhs: &'a Reference,
    rhs: &'a Reference,
) -> Option<(PackedState<'a>, ComponentsState<'a>)> {
    if lhs.ptr != rhs.ptr {
        return None;
    }
    let packed = match (
        lhs.get_repr_as::<TuplePacked>(),
        rhs.get_repr_as::<TuplePacked>(),
    ) {
        (Some(lhs), None) => lhs,
        (None, Some(rhs)) => rhs,
        _ => return None,
    };
    let atom = match (
        lhs.get_repr_as::<TupleComponents>(),
        rhs.get_repr_as::<TupleComponents>(),
    ) {
        (Some(lhs), None) => lhs,
        (None, Some(rhs)) => rhs,
        _ => return None,
    };
    Some((packed, atom))
}

#[cfg(test)]
mod tests {
    use super::*;
    use conjure_cp::ast::Domain;
    use conjure_cp::representation::ReprRule;
    use conjure_cp::rule_engine::get_rule_by_name;
    use conjure_cp::{domain_int, range};

    /// Regression: a packed tuple field that is itself compound (here, a nested tuple) used to
    /// decode as `SafeIndex` into a matrix literal of compound (tuple) elements, dynamically
    /// indexed by a decision-variable-derived digit -- a shape the Minion backend cannot turn
    /// into a per-field `Element` constraint chain (it errors with "expected a literal but got
    /// `AbstractLiteral(...)`"). The field must come back as an inline tuple literal instead,
    /// decoded recursively field-by-field, so later rules (`tuple_literal_index`,
    /// `tuple_literal_eq_literal`) can keep resolving it.
    #[test]
    fn indexing_a_compound_field_rebuilds_it_as_a_tuple_literal() {
        let inner_domain = Domain::tuple(vec![domain_int!(7..8), Domain::bool()]);
        let domain = Domain::tuple(vec![inner_domain, domain_int!(13..17)]);
        let mut symbols = SymbolTable::new();
        let mut declaration = symbols.gen_find(&domain);
        TuplePacked::init_for(&mut declaration).unwrap();

        let mut reference = Reference::new(declaration);
        let _ = reference.select_repr::<TuplePacked>().unwrap();
        let subject = Expr::from(reference);
        let expr = Expr::SafeIndex(Metadata::new(), Moo::new(subject), vec![1.into()]);

        let rule = get_rule_by_name("tuple_packed_index_lit").expect("rule registered");
        let result = rule.apply(&expr, &symbols).expect("should index field 1");

        let Expr::AbstractLiteral(_, AbstractLiteral::Tuple(fields)) = &result.new_expression
        else {
            panic!(
                "expected the compound field to come back as an inline tuple literal, got {}",
                result.new_expression
            );
        };
        assert_eq!(fields.len(), 2);
    }
}
