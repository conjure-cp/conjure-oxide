use crate::guard;
use crate::representation::record_to_atom::RecordToAtom;
use crate::utils::{is_record_lit, record_expr_entries};
use conjure_cp::ast::{
    Atom, Expression, HasDomain, Metadata, Moo, Name, RecordEntry, Reference, SymbolTable,
};
use conjure_cp::representation::ReprRule;
use conjure_cp::rule_engine::{
    ApplicationError::RuleNotApplicable, ApplicationResult, Reduction, register_rule,
};
use conjure_cp::{bug, essence_expr, into_matrix_expr};
use itertools::Itertools;

/// Indexing into a record variable
/// e.g:
/// ```plain
/// x[a]
/// ~>
/// x_RecordToAtom_1
/// ```
/// where
/// ```plain
/// x: record { a : bool, b : int(0..9) }
/// ```
#[register_rule(("Base", 2000))]
fn index_record_to_atom(expr: &Expression, _: &SymbolTable) -> ApplicationResult {
    let (subject, indices, safe) = match expr {
        Expression::UnsafeIndex(_, subject, indices) => (subject, indices, false),
        Expression::SafeIndex(_, subject, indices) => (subject, indices, true),
        _ => return Err(RuleNotApplicable),
    };

    guard!(
        let Expression::Atomic(_, Atom::Reference(re)) = &**subject &&
        let Some(repr) = RecordToAtom::get_for(&re.ptr) &&
        let Some(Expression::Atomic(_, Atom::Reference(idx_re))) = indices.first() &&
        let Some((field_name, _)) = idx_re.ptr.as_record_field()
        else {
            return Err(RuleNotApplicable);
        }
    );

    let field_atom = repr
        .elems
        .get(&field_name)
        .unwrap_or_else(|| bug!("field {} does not exist in {}", field_name, subject));

    let lhs = Reference::from(field_atom.clone());
    let rhs = &indices[1..];

    if rhs.is_empty() {
        Ok(Reduction::pure(lhs.into()))
    } else if safe {
        let new_expr = Expression::SafeIndex(Metadata::new(), Moo::new(lhs.into()), rhs.into());
        Ok(Reduction::pure(new_expr))
    } else {
        let new_expr = Expression::UnsafeIndex(Metadata::new(), Moo::new(lhs.into()), rhs.into());
        Ok(Reduction::pure(new_expr))
    }
}

/// Equality of two record variables
/// e.g:
/// ```plain
/// x == y
/// ~>
/// x[a] == y[a] /\ x[b] == y[b]
/// ```
/// where
/// ```plain
/// x, y: record { a : bool, b : int(0..9) }
/// ```
#[register_rule(("Base", 2000))]
fn record_var_equality(expr: &Expression, _: &SymbolTable) -> ApplicationResult {
    guard!(
        let Expression::Eq(_, left, right) = expr &&
        let Expression::Atomic(_, Atom::Reference(decl)) = &**left &&
        let Some(repr) = RecordToAtom::get_for(&decl.ptr) &&
        let Expression::Atomic(_, Atom::Reference(decl2)) = &**right &&
        let Some(repr2) = RecordToAtom::get_for(&decl2.ptr) &&
        let Some(entries) = decl.domain_of().as_record() &&
        let Some(entries2) = decl.domain_of().as_record()
        else {
            return Err(RuleNotApplicable);
        }
    );

    // TODO: clarify semantics; maybe this should just return false instead?
    let valid = entries.len() == entries2.len();
    if !valid {
        bug!("equality on records with different shapes: {expr}")
    }
    let names = entries
        .iter()
        .map(|RecordEntry { name, .. }| name)
        .collect_vec();

    let mut equality_constraints = vec![];
    for name in names {
        // we are literally iterating left's names, so if this fails, something is definitely broken
        let f1 = repr
            .field_ref(name)
            .unwrap_or_else(|| bug!("record {left} has no field {name}"));

        // TODO: rhs is different shape; should we fail, not apply, or evaluate to false?
        let f2 = repr2
            .field_ref(name)
            .unwrap_or_else(|| bug!("equality on records with different shapes: {expr}"));

        equality_constraints.push(essence_expr!(&f1 == &f2));
    }

    let new_expr = Expression::And(
        Metadata::new(),
        Moo::new(into_matrix_expr!(equality_constraints)),
    );

    Ok(Reduction::pure(new_expr))
}

/// Equality of record variable to record literal/expression
/// e.g:
/// ```plain
/// x == { a: true, b: N + 1 }
/// ~>
/// x[a] == true /\ x[b] == N + 1
/// ```
/// where
/// ```plain
/// x, y: record { a : bool, b : int(0..9) }
/// ```
#[register_rule(("Base", 2000))]
fn record_equality_var_to_lit(expr: &Expression, _: &SymbolTable) -> ApplicationResult {
    guard!(
        let Expression::Eq(_, lhs, rhs) = expr &&
        let Expression::Atomic(_, Atom::Reference(re)) = lhs.as_ref() &&
        let Some(repr) = RecordToAtom::get_for(&re.ptr) &&
        let Some(rhs_ents) = record_expr_entries(rhs)
        else {
            return Err(RuleNotApplicable);
        }
    );

    // unroll the equality into equality constraints for each field
    let mut equality_constraints = vec![];
    for (name, expr) in rhs_ents {
        // TODO: literal is different shape from the variable; should we fail, not apply, or evaluate to false?
        let field = repr
            .field_ref(name)
            .unwrap_or_else(|| bug!("equality on records with different shapes: {expr}"));
        equality_constraints.push(essence_expr!(&field == &expr));
    }

    let new_expr = Expression::And(
        Metadata::new(),
        Moo::new(into_matrix_expr!(equality_constraints)),
    );

    Ok(Reduction::pure(new_expr))
}

/// If we have a record literal on the left and variable on the right, swap them
/// so the above rule can apply
/// e.g:
/// ```plain
/// { ... } = x
/// ~>
/// x == { ... }
/// ```
#[register_rule(("Base", 2001))]
fn record_equality_reorder(expr: &Expression, _: &SymbolTable) -> ApplicationResult {
    guard!(
        let Expression::Eq(md, lit, var) = expr &&
        let Expression::Atomic(_, Atom::Reference(re)) = var.as_ref() &&
        is_record_lit(lit.as_ref())
        else {
            return Err(RuleNotApplicable);
        }
    );

    Ok(Reduction::pure(Expression::Eq(
        md.clone(),
        var.clone(),
        lit.clone(),
    )))
}
