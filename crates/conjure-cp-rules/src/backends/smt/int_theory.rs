//! Materialising the SMT integer theories.
//!
//! [`SmtLia`] and [`SmtBv`] do not decompose a variable -- they leave one variable of the same
//! domain -- so all that is needed here is to point references at the variable the representation
//! introduced. The adaptor then reads the theory back off that variable's representation and gives
//! it the matching Z3 sort.
//!
//! A declaration carrying both representations gets a channelling equality between the two from the
//! representation machinery, which lands here as an equality between the two variables; the adaptor
//! bridges the sorts.

use conjure_cp::ast::{Atom, Expression as Expr, Reference, SymbolTable};
use conjure_cp::rule_engine::{
    ApplicationError::RuleNotApplicable, ApplicationResult, RuleEffect, register_rule,
};

use crate::types::int::{SmtBv, SmtLia};
use crate::types::matrix::MatrixArray;

/// Point a reference at the variable its linear-arithmetic representation introduced.
#[register_rule("Smt", 9500, [Atomic])]
fn smt_int_lia_variable(expr: &Expr, _: &SymbolTable) -> ApplicationResult {
    let Expr::Atomic(_, Atom::Reference(reference)) = expr else {
        return Err(RuleNotApplicable);
    };
    let state = reference.get_repr_as::<SmtLia>().ok_or(RuleNotApplicable)?;
    Ok(RuleEffect::pure(Reference::new(state.value.clone()).into()))
}

/// Point a reference at the variable its bit-vector representation introduced.
#[register_rule("Smt", 9500, [Atomic])]
fn smt_int_bv_variable(expr: &Expr, _: &SymbolTable) -> ApplicationResult {
    let Expr::Atomic(_, Atom::Reference(reference)) = expr else {
        return Err(RuleNotApplicable);
    };
    let state = reference.get_repr_as::<SmtBv>().ok_or(RuleNotApplicable)?;
    Ok(RuleEffect::pure(Reference::new(state.value.clone()).into()))
}

/// Point a reference at the array its layout representation introduced.
///
/// The array's element sort is not decided here: the variable this leaves behind goes on to pick
/// `lia` or `bv` for its integers like any other declaration.
#[register_rule("Smt", 9500, [Atomic])]
fn smt_matrix_array_variable(expr: &Expr, _: &SymbolTable) -> ApplicationResult {
    let Expr::Atomic(_, Atom::Reference(reference)) = expr else {
        return Err(RuleNotApplicable);
    };
    let state = reference
        .get_repr_as::<MatrixArray>()
        .ok_or(RuleNotApplicable)?;
    Ok(RuleEffect::pure(Reference::new(state.value.clone()).into()))
}
