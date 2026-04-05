use crate::guard;
use crate::representation::tuple_packed::TuplePacked;
use crate::representation::tuple_to_atom::TupleToAtom;
use crate::utils::{as_cmp_or_lex_op, as_eq_or_neq, eq_or_neq, tuple_expr_entries};
use conjure_cp::ast::{
    Atom, DomainPtr, Expression as Expr, GroundDomain, HasDomain, Literal, Metadata, Moo, Range,
    Reference, SymbolTable,
};
use conjure_cp::bug_assert;
use conjure_cp::essence_expr;
use conjure_cp::representation::ReprRule;
use conjure_cp::rule_engine::ApplicationError::RuleNotApplicable;
use conjure_cp::rule_engine::{ApplicationResult, Reduction, register_rule, register_rule_set};
use parking_lot::MappedRwLockReadGuard;
use std::collections::VecDeque;
use uniplate::Biplate;

register_rule_set!("ReprTuplePacked", ("Base"));

/// Select the TuplePacked representation for comparison operations on integer tuples.
/// Higher priority than the general `select_representation` (8000) and
/// `uniform_repr_in_comparison_op` (9000).
#[register_rule(("ReprTuplePacked", 9500))]
fn select_packed_for_comparison(expr: &Expr, _: &SymbolTable) -> ApplicationResult {
    guard!(
        let Some((lhs, rhs)) = as_cmp_or_lex_op(expr) else {
            return Err(RuleNotApplicable)
        }
    );

    let lhs_needs = needs_packed_repr(lhs.as_ref());
    let rhs_needs = needs_packed_repr(rhs.as_ref());

    if !lhs_needs && !rhs_needs {
        return Err(RuleNotApplicable);
    }

    let mut new_lhs: Expr = (*lhs).clone();
    let mut new_rhs: Expr = (*rhs).clone();
    let mut all_symbols = SymbolTable::new();
    let mut all_constraints = Vec::new();

    if lhs_needs {
        let (symbols, constraints) = init_packed_repr(&mut new_lhs)?;
        all_symbols.extend(symbols);
        all_constraints.extend(constraints);
    }
    if rhs_needs {
        let (symbols, constraints) = init_packed_repr(&mut new_rhs)?;
        all_symbols.extend(symbols);
        all_constraints.extend(constraints);
    }

    let new_expr = expr.with_children_bi(VecDeque::from([Moo::new(new_lhs), Moo::new(new_rhs)]));
    Ok(Reduction::new(new_expr, all_constraints, all_symbols))
}

/// Equality of packed tuple variables
/// ```plain
/// x = y  (both TuplePacked)  ~>  x_packed = y_packed
/// ```
#[register_rule(("ReprTuplePacked", 2000))]
fn tuple_packed_var_eq_var(expr: &Expr, _: &SymbolTable) -> ApplicationResult {
    let (lhs, rhs, neq) = as_eq_or_neq(expr)?;

    guard!(
        let Expr::Atomic(_, Atom::Reference(re))  = lhs &&
        let Some(lp) = re.get_repr_as::<TuplePacked>()  &&
        let Expr::Atomic(_, Atom::Reference(re2)) = rhs &&
        let Some(rp) = re2.get_repr_as::<TuplePacked>()
        else { return Err(RuleNotApplicable) }
    );

    let new_expr = eq_or_neq(neq, lp.packed_expr(), rp.packed_expr());
    Ok(Reduction::pure(new_expr))
}

/// Equality of packed tuple variable to a tuple literal
/// ```plain
/// x = (1, 2, 3)  (x is TuplePacked)  ~>  x_packed = encode(1,2,3)
/// ```
#[register_rule(("ReprTuplePacked", 2000))]
fn tuple_packed_var_eq_lit(expr: &Expr, _: &SymbolTable) -> ApplicationResult {
    let (lhs, rhs, neq) = as_eq_or_neq(expr)?;

    guard!(
        let Expr::Atomic(_, Atom::Reference(re)) = lhs   &&
        let Some(repr) = re.get_repr_as::<TuplePacked>() &&
        let Some(rhs_ents) = tuple_expr_entries(rhs)
        else { return Err(RuleNotApplicable) }
    );

    bug_assert!(
        repr.sizes.len() == rhs_ents.len(),
        "equality on tuples with different shapes!"
    );

    let packed_val = repr.encode_lit_entries(&rhs_ents)?;
    let new_expr = eq_or_neq(neq, repr.packed_expr(), packed_val);
    Ok(Reduction::pure(new_expr))
}

/// Comparison of packed tuple variables
/// ```plain
/// x > y  (both TuplePacked)  ~>  x_packed > y_packed
/// ```
#[register_rule(("ReprTuplePacked", 2000))]
fn tuple_packed_var_cmp_var(expr: &Expr, _: &SymbolTable) -> ApplicationResult {
    guard!(
        as_eq_or_neq(expr).is_err() &&
        let Some((lhs, rhs)) = as_cmp_or_lex_op(expr)               &&
        let Expr::Atomic(_, Atom::Reference(lhs_re)) = lhs.as_ref() &&
        let Expr::Atomic(_, Atom::Reference(rhs_re)) = rhs.as_ref() &&
        let Some(lp) = lhs_re.get_repr_as::<TuplePacked>()          &&
        let Some(rp) = rhs_re.get_repr_as::<TuplePacked>()
        else { return Err(RuleNotApplicable) }
    );

    Ok(Reduction::pure(packed_cmp(
        expr,
        lp.packed_expr(),
        rp.packed_expr(),
    )))
}

/// Comparison of packed tuple variable to a literal
/// ```plain
/// x > (1,2,3)  (x is TuplePacked)  ~>  x_packed > encode(1,2,3)
/// ```
#[register_rule(("ReprTuplePacked", 2000))]
fn tuple_packed_var_cmp_lit(expr: &Expr, _: &SymbolTable) -> ApplicationResult {
    guard!(
        as_eq_or_neq(expr).is_err() &&
        let Some((lhs, rhs)) = as_cmp_or_lex_op(expr)               &&
        let Expr::Atomic(_, Atom::Reference(lhs_re)) = lhs.as_ref() &&
        let Some(repr) = lhs_re.get_repr_as::<TuplePacked>()        &&
        let Some(rhs_ents) = tuple_expr_entries(&rhs)
        else { return Err(RuleNotApplicable) }
    );

    let packed_val = repr.encode_lit_entries(&rhs_ents)?;
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
#[register_rule(("ReprTuplePacked", 2000))]
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
    bug_assert!(i < repr.sizes.len(), "tuple indexing is out of bounds");

    let packed = repr.packed_expr();
    let stride = repr.strides[i];
    let size = repr.sizes[i];
    let min = repr.mins[i];

    // (packed / stride) % size + min
    let new_expr = match (stride, i) {
        (1, _) if size == repr.total_size => essence_expr!(&packed + &min),
        (1, _) => essence_expr!((&packed % &size) + &min),
        (_, 0) => essence_expr!((&packed / &stride) + &min),
        _ => essence_expr!(((&packed / &stride) % &size) + &min),
    };

    let remaining = &indices[1..];
    if remaining.is_empty() {
        Ok(Reduction::pure(new_expr))
    } else {
        Ok(Reduction::pure(Expr::SafeIndex(
            Metadata::new(),
            new_expr.into(),
            Vec::from(remaining),
        )))
    }
}

/// Channeling constraint between TupleToAtom and TuplePacked for the same variable.
/// Handles equalities of the form `x#TupleToAtom = x#TuplePacked` (or reversed).
/// ```plain
/// x#TupleToAtom = x#TuplePacked
/// ~>
/// x_packed = sum_i (x_TupleToAtom_i - min_i) * stride_i
/// ```
#[register_rule(("ReprTuplePacked", 3000))]
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

    // Build: packed = sum_i (atom_i - min_i) * stride_i
    let sum_expr = atom
        .elems
        .iter()
        .enumerate()
        .map(|(i, decl)| {
            let elem: Expr = Reference::new(decl.clone()).into();
            let offset = match packed.mins[i] {
                0 => elem,
                min_i => essence_expr!(&elem - &min_i),
            };
            match packed.strides[i] {
                1 => offset,
                stride_i => essence_expr!(&offset * &stride_i),
            }
        })
        .reduce(|acc: Expr, part: Expr| essence_expr!(&acc + &part))
        .unwrap();

    Ok(Reduction::pure(eq_or_neq(
        neq,
        packed.packed_expr(),
        sum_expr,
    )))
}

/// True if `expr` is an unrepresented reference to a packable integer tuple.
fn needs_packed_repr(expr: &Expr) -> bool {
    matches!(
        expr,
        Expr::Atomic(_, Atom::Reference(re))
            if re.repr.is_none() && is_packable_tuple_domain(&re.domain_of())
    )
}

/// Select TuplePacked for the reference inside `expr`, returning symbols and constraints.
fn init_packed_repr(
    expr: &mut Expr,
) -> Result<(SymbolTable, Vec<Expr>), conjure_cp::rule_engine::ApplicationError> {
    if let Expr::Atomic(_, Atom::Reference(re)) = expr {
        let (_, symbols, constraints) = re
            .select_or_init_repr::<TuplePacked>()
            .map_err(|_| RuleNotApplicable)?;
        Ok((symbols, constraints))
    } else {
        Err(RuleNotApplicable)
    }
}

/// Check if a domain is a packable tuple of bounded integers fitting in i32.
/// Accepts both contiguous and non-contiguous ("holey") integer domains.
fn is_packable_tuple_domain(domain: &DomainPtr) -> bool {
    let Some(gd_tuple) = domain.as_tuple_ground() else {
        return false;
    };

    let mut total: i64 = 1;
    for elem_dom in gd_tuple {
        guard!(
            let GroundDomain::Int(ranges) = elem_dom.as_ref() &&
            let Some(span) = Range::spanning(ranges).length() &&
            span > 0
            else {
                return false;
            }
        );

        total = total.saturating_mul(span as i64);
        if total > i32::MAX as i64 {
            return false;
        }
    }

    total > 0
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

type PackedState<'a> = MappedRwLockReadGuard<'a, <TuplePacked as ReprRule>::DeclLevel>;
type ToAtomState<'a> = MappedRwLockReadGuard<'a, <TupleToAtom as ReprRule>::DeclLevel>;
fn as_channeling_pair<'a>(
    lhs: &'a Reference,
    rhs: &'a Reference,
) -> Option<(PackedState<'a>, ToAtomState<'a>)> {
    let packed = match (
        lhs.get_repr_as::<TuplePacked>(),
        rhs.get_repr_as::<TuplePacked>(),
    ) {
        (Some(lhs), None) => lhs,
        (None, Some(rhs)) => rhs,
        _ => return None,
    };
    let atom = match (
        lhs.get_repr_as::<TupleToAtom>(),
        rhs.get_repr_as::<TupleToAtom>(),
    ) {
        (Some(lhs), None) => lhs,
        (None, Some(rhs)) => rhs,
        _ => return None,
    };
    Some((packed, atom))
}
