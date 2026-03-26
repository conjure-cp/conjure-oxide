use crate::guard;
use crate::representation::record_to_atom::RecordToAtom;
use crate::utils::{as_eq_or_neq, collect_eq_or_neq, is_record_lit, record_expr_entries};
use conjure_cp::ast::{
    Atom, Expression, HasDomain, Metadata, Moo, RecordEntry, Reference, SymbolTable,
};
use conjure_cp::bug;
use conjure_cp::representation::ReprRule;
use conjure_cp::rule_engine::{
    ApplicationError::RuleNotApplicable, ApplicationResult, Reduction, register_rule,
};

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
#[register_rule(("ReprGeneral", 2000))]
fn index_record_to_atom(expr: &Expression, _: &SymbolTable) -> ApplicationResult {
    let (subject, indices) = match expr {
        Expression::UnsafeIndex(_, subject, indices) => (subject, indices),
        Expression::SafeIndex(_, subject, indices) => (subject, indices),
        _ => return Err(RuleNotApplicable),
    };

    guard!(
        let Expression::Atomic(_, Atom::Reference(re)) = &**subject                &&
        let Some(repr) = re.get_repr_as::<RecordToAtom>()                          &&
        let Some(Expression::Atomic(_, Atom::Reference(idx_re))) = indices.first() &&
        let Some((field_name, _)) = idx_re.ptr.as_record_field()
        else {
            return Err(RuleNotApplicable);
        }
    );

    assert_eq!(
        indices.len(),
        1,
        "record indexing is always one dimensional"
    );

    let field_atom = repr
        .elems
        .get(&field_name)
        .unwrap_or_else(|| bug!("field {} does not exist in {}", field_name, subject));

    let lhs = Reference::from(field_atom.clone());
    Ok(Reduction::pure(lhs.into()))
}

/// (In)Equality of two record variables
/// e.g:
/// ```plain
/// x == y
/// ~>
/// x[a] == y[a] /\ x[b] == y[b]
/// ```
/// where
/// ```plainW
/// x, y: record { a : bool, b : int(0..9) }
/// ```
#[register_rule(("ReprGeneral", 2000))]
fn record_var_eq_var(expr: &Expression, _: &SymbolTable) -> ApplicationResult {
    let (lhs, rhs, neq) = as_eq_or_neq(expr)?;

    guard!(
        let Expression::Atomic(_, Atom::Reference(re)) = lhs   &&
        let Some(repr) = re.get_repr_as::<RecordToAtom>()      &&
        let Expression::Atomic(_, Atom::Reference(re2)) = rhs  &&
        let Some(repr2) = re2.get_repr_as::<RecordToAtom>()    &&
        let Some(entries) = re.domain_of().as_record()         &&
        let Some(entries2) = re2.domain_of().as_record()
        else {
            return Err(RuleNotApplicable);
        }
    );

    // TODO: clarify semantics; should we fail, not apply, or evaluate to false?
    if !entries.len() == entries2.len() {
        bug!("equality on records with different shapes: {expr}")
    }

    let equalities = entries.iter().map(|RecordEntry { name, .. }| {
        let f1 = repr
            .field_ref(name)
            .unwrap_or_else(|| bug!("record {lhs} has no field {name}"));

        // TODO: rhs is different shape; should we fail, not apply, or evaluate to false?
        let f2 = repr2
            .field_ref(name)
            .unwrap_or_else(|| bug!("equality on records with different shapes: {expr}"));

        (f1, f2)
    });

    let new_expr = collect_eq_or_neq(neq, equalities);
    Ok(Reduction::pure(new_expr))
}

/// (In)Equality of record variable to record literal/expression
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
#[register_rule(("ReprGeneral", 2000))]
fn record_var_eq_lit(expr: &Expression, _: &SymbolTable) -> ApplicationResult {
    let (lhs, rhs, neq) = as_eq_or_neq(expr)?;

    guard!(
        let Expression::Atomic(_, Atom::Reference(re)) = lhs &&
        let Some(repr) = re.get_repr_as::<RecordToAtom>()    &&
        let Some(rhs_ents) = record_expr_entries(rhs)
        else {
            return Err(RuleNotApplicable);
        }
    );

    let equalities = rhs_ents.map(|(name, expr)| {
        let field = repr
            .field_ref(name)
            .unwrap_or_else(|| bug!("equality on records with different shapes: {expr}"));

        (field, expr)
    });

    let new_expr = collect_eq_or_neq(neq, equalities);
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
#[register_rule(("ReprGeneral", 2001))]
fn record_eq_reorder(expr: &Expression, _: &SymbolTable) -> ApplicationResult {
    guard!(
        let Expression::Eq(md, lit, var) = expr                      &&
        let Expression::Atomic(_, Atom::Reference(_)) = var.as_ref() &&
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
