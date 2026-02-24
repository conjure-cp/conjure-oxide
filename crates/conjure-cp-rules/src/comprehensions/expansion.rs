//! Comprehension expansion rules

mod expand_native;
mod expand_via_solver;
mod expand_via_solver_ac;
mod via_solver_common;

pub use expand_native::expand_native;
pub use expand_via_solver::expand_via_solver;
pub use expand_via_solver_ac::expand_via_solver_ac;

use std::collections::VecDeque;

use conjure_cp::{
    ast::{Expression as Expr, SymbolTable, comprehension::Comprehension},
    into_matrix_expr,
    rule_engine::{
        ApplicationError::RuleNotApplicable, ApplicationResult, Reduction, register_rule,
    },
    settings::{QuantifiedExpander, comprehension_expander},
};
use uniplate::Biplate;

use uniplate::Uniplate;

/// Expand comprehensions inside AC operators using `--comprehension-expander via-solver-ac`.
#[register_rule(("Base", 2002))]
fn expand_comprehension_via_solver_ac(expr: &Expr, symbols: &SymbolTable) -> ApplicationResult {
    if comprehension_expander() != QuantifiedExpander::ViaSolverAc {
        return Err(RuleNotApplicable);
    }

    // Is this an ac expression?
    let ac_operator_kind = expr.to_ac_operator_kind().ok_or(RuleNotApplicable)?;

    debug_assert_eq!(
        expr.children().len(),
        1,
        "AC expressions should have exactly one child."
    );

    let Expr::Comprehension(_, ref comprehension) = expr.children()[0] else {
        return Err(RuleNotApplicable);
    };

    // unwrap comprehensions inside out. This reduces calls to minion when rewriting nested
    // comprehensions.
    let nested_comprehensions: VecDeque<Comprehension> =
        (**comprehension).clone().return_expression().universe_bi();
    if !nested_comprehensions.is_empty() {
        return Err(RuleNotApplicable);
    };

    // TODO: check what kind of error this throws and maybe panic
    let mut symbols = symbols.clone();
    let results = expand_via_solver_ac((**comprehension).clone(), &mut symbols, ac_operator_kind)
        .or(Err(RuleNotApplicable))?;

    let new_expr = ac_operator_kind.as_expression(into_matrix_expr!(results));
    Ok(Reduction::with_symbols(new_expr, symbols))
}

/// Expand comprehensions using `--comprehension-expander native`.
#[register_rule(("Base", 2000))]
fn expand_comprehension_native(expr: &Expr, symbols: &SymbolTable) -> ApplicationResult {
    if comprehension_expander() != QuantifiedExpander::Native {
        return Err(RuleNotApplicable);
    }

    let Expr::Comprehension(_, comprehension) = expr else {
        return Err(RuleNotApplicable);
    };

    // unwrap comprehensions inside out. This reduces calls to minion when rewriting nested
    // comprehensions.
    let nested_comprehensions: VecDeque<Comprehension> =
        (**comprehension).clone().return_expression().universe_bi();
    if !nested_comprehensions.is_empty() {
        return Err(RuleNotApplicable);
    };

    // TODO: check what kind of error this throws and maybe panic

    let mut symbols = symbols.clone();
    let results =
        expand_native(comprehension.as_ref().clone(), &mut symbols).or(Err(RuleNotApplicable))?;

    Ok(Reduction::with_symbols(into_matrix_expr!(results), symbols))
}

/// Expand comprehensions using `--comprehension-expander via-solver` (and as fallback for
/// non-AC comprehensions when `--comprehension-expander via-solver-ac`).
#[register_rule(("Base", 2000))]
fn expand_comprehension_via_solver(expr: &Expr, symbols: &SymbolTable) -> ApplicationResult {
    if !matches!(
        comprehension_expander(),
        QuantifiedExpander::ViaSolver | QuantifiedExpander::ViaSolverAc
    ) {
        return Err(RuleNotApplicable);
    }

    let Expr::Comprehension(_, comprehension) = expr else {
        return Err(RuleNotApplicable);
    };

    // unwrap comprehensions inside out. This reduces calls to minion when rewriting nested
    // comprehensions.
    let nested_comprehensions: VecDeque<Comprehension> =
        (**comprehension).clone().return_expression().universe_bi();
    if !nested_comprehensions.is_empty() {
        return Err(RuleNotApplicable);
    };

    // TODO: check what kind of error this throws and maybe panic

    let mut symbols = symbols.clone();
    let results = expand_via_solver(comprehension.as_ref().clone(), &mut symbols)
        .or(Err(RuleNotApplicable))?;

    Ok(Reduction::with_symbols(into_matrix_expr!(results), symbols))
}
