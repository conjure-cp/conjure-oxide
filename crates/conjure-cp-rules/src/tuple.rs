use crate::guard;
use crate::representation::tuple_to_atom::TupleToAtom;
use crate::utils::{as_eq_or_neq, collect_eq_or_neq, is_tuple_lit, tuple_expr_entries};
use conjure_cp::ast::{
    Atom, Expression as Expr, Expression, HasDomain, Literal, Metadata, Reference, SymbolTable,
};
use conjure_cp::bug_assert_eq;
use conjure_cp::rule_engine::ApplicationError::RuleNotApplicable;
use conjure_cp::rule_engine::{ApplicationResult, Reduction, register_rule};
use conjure_cp::{bug_assert, essence_expr};
use itertools::izip;
use conjure_cp::ast::pretty::pretty_vec;

/// Indexing into a tuple variable
/// ```plain
/// x[1]
/// ~>
/// x_TupleToAtom_1
/// ```
#[register_rule(("ReprGeneral", 2000))]
fn tuple_to_atom_index_lit(expr: &Expr, _: &SymbolTable) -> ApplicationResult {
    guard!(
        let Expr::SafeIndex(_, subject, indices) = expr        &&
        let Expr::Atomic(_, Atom::Reference(re)) = &**subject  &&
        let Some(Expr::Atomic(_, idx)) = indices.first()       &&
        let Atom::Literal(Literal::Int(idx)) = idx             &&
        let Some(repr) = re.get_repr_as::<TupleToAtom>()
        else {
            return Err(RuleNotApplicable);
        }
    );
    let idx = (*idx - 1) as usize;
    bug_assert!(idx < repr.elems.len(), "tuple indexing is out of bounds");

    let lhs = Reference::new(repr.elems[idx].clone());
    let rhs = &indices[1..];

    if rhs.is_empty() {
        Ok(Reduction::pure(lhs.into()))
    } else {
        let new_expr = Expr::SafeIndex(Metadata::new(), lhs.into(), Vec::from(rhs));
        Ok(Reduction::pure(new_expr))
    }
}

/// Convert an unsafe tuple index into a safe one
/// ```plain
/// x[y]
/// ~>
/// { x[y] @ (y >= 1 /\ y <= |x|) }
/// ```
#[register_rule(("Bubble", 8000))]
fn tuple_index_to_bubble(expr: &Expr, _: &SymbolTable) -> ApplicationResult {
    guard!(
        let Expr::UnsafeIndex(_, subject, indices) = expr &&
        let Some(idx) = indices.first()                   &&
        let Some(idx_dom) = idx.domain_of()               &&
        let Some(dom) = subject.domain_of()               &&
        let Some(inner_doms) = dom.as_tuple()
        else {
            return Err(RuleNotApplicable);
        }
    );
    bug_assert!(
        idx_dom.as_int().is_some(),
        "tuple indexing expression must be integer"
    );

    let len = inner_doms.len() as i32;
    let bubble_cond = essence_expr!(r"(&idx >= 1) /\ (&idx <= &len)");
    let bubble_expr = Expr::SafeIndex(Metadata::new(), subject.clone(), indices.clone());

    let new_expr = Expr::Bubble(Metadata::new(), bubble_expr.into(), bubble_cond.into());
    Ok(Reduction::pure(new_expr))
}

/// Equality of tuple variables
/// ```plain
/// x = y
/// ~>
/// x[1] = y[1] /\ x[2] = y[2] /\ ... /\ x[N] = y[N]
/// ```
#[register_rule(("ReprGeneral", 2000))]
fn tuple_var_eq_var(expr: &Expr, _: &SymbolTable) -> ApplicationResult {
    let (lhs, rhs, neq) = as_eq_or_neq(expr)?;

    guard!(
        let Expr::Atomic(_, Atom::Reference(re)) = lhs     &&
        let Some(repr) = re.get_repr_as::<TupleToAtom>()   &&
        let Expr::Atomic(_, Atom::Reference(re2)) = rhs    &&
        let Some(repr2) = re2.get_repr_as::<TupleToAtom>()
        else {
            return Err(RuleNotApplicable);
        }
    );

    bug_assert_eq!(
        repr.elems.len(),
        repr2.elems.len(),
        "equality on tuples with different shapes!"
    );

    let new_expr = collect_eq_or_neq(neq, izip!(repr.elem_refs(), repr2.elem_refs()));
    Ok(Reduction::pure(new_expr))
}

/// Equality of tuple variable to a tuple literal
/// ```plain
/// x = (1, true)
/// ~>
/// x[1] = 1 /\ x[2] = true
/// ```
#[register_rule(("ReprGeneral", 2000))]
fn tuple_var_eq_lit(expr: &Expr, _: &SymbolTable) -> ApplicationResult {
    let (lhs, rhs, neq) = as_eq_or_neq(expr)?;

    guard!(
        let Expr::Atomic(_, Atom::Reference(re)) = lhs   &&
        let Some(repr) = re.get_repr_as::<TupleToAtom>() &&
        let Some(rhs_ents) = tuple_expr_entries(rhs)
        else {
            return Err(RuleNotApplicable);
        }
    );

    bug_assert_eq!(
        repr.elems.len(),
        rhs_ents.len(),
        "equality on tuples with different shapes!"
    );

    let new_expr = collect_eq_or_neq(neq, izip!(repr.elem_refs(), rhs_ents));
    Ok(Reduction::pure(new_expr))
}

/// If we have a tuple literal on the left and variable on the right, swap them
/// so the above rule can apply
#[register_rule(("ReprGeneral", 2001))]
fn tuple_eq_reorder(expr: &Expr, _: &SymbolTable) -> ApplicationResult {
    guard!(
        let Expression::Eq(_, lit, var) = expr                       &&
        let Expression::Atomic(_, Atom::Reference(_)) = var.as_ref() &&
        is_tuple_lit(lit.as_ref())
        else {
            return Err(RuleNotApplicable);
        }
    );

    Ok(Reduction::pure(Expression::Eq(
        Metadata::new(),
        var.clone(),
        lit.clone(),
    )))
}
