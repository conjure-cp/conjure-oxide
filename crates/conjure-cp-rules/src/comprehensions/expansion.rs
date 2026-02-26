//! Comprehension expansion rules

mod expand_native;
mod expand_via_solver;
mod expand_via_solver_ac;
mod via_solver_common;

pub use expand_native::expand_native;
pub use expand_via_solver::expand_via_solver;
pub use expand_via_solver_ac::expand_via_solver_ac;

use conjure_cp::{
    ast::{Expression as Expr, SymbolTable, comprehension::Comprehension},
    bug, into_matrix_expr,
    rule_engine::{
        ApplicationError::RuleNotApplicable, ApplicationResult, Reduction, register_rule,
    },
    settings::{QuantifiedExpander, comprehension_expander},
};
use uniplate::Uniplate;

/// Expand comprehensions using `--comprehension-expander native`.
#[register_rule(("Base", 2000))]
fn expand_comprehension_native(expr: &Expr, symbols: &SymbolTable) -> ApplicationResult {
    if comprehension_expander() != QuantifiedExpander::Native {
        return Err(RuleNotApplicable);
    }

    let Expr::Comprehension(_, comprehension) = expr else {
        return Err(RuleNotApplicable);
    };

    let comprehension = comprehension.as_ref().clone();
    let mut symbols = symbols.clone();
    let results = expand_native(comprehension, &mut symbols)
        .unwrap_or_else(|e| bug!("native comprehension expansion failed: {e}"));
    Ok(Reduction::with_symbols(into_matrix_expr!(results), symbols))
}

/// Expand comprehensions using `--comprehension-expander via-solver`.
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

    let comprehension = comprehension.as_ref().clone();
    let results = expand_via_solver(comprehension)
        .unwrap_or_else(|e| bug!("via-solver comprehension expansion failed: {e}"));
    Ok(Reduction::with_symbols(
        into_matrix_expr!(results),
        symbols.clone(),
    ))
}

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

    let comprehension = as_single_comprehension(&expr.children()[0]).ok_or(RuleNotApplicable)?;

    let results =
        expand_via_solver_ac(comprehension, ac_operator_kind).or(Err(RuleNotApplicable))?;

    let new_expr = ac_operator_kind.as_expression(into_matrix_expr!(results));
    Ok(Reduction::with_symbols(new_expr, symbols.clone()))
}

fn as_single_comprehension(expr: &Expr) -> Option<Comprehension> {
    if let Expr::Comprehension(_, comprehension) = expr {
        return Some(comprehension.as_ref().clone());
    }

    let exprs = expr.clone().unwrap_list()?;
    let [Expr::Comprehension(_, comprehension)] = exprs.as_slice() else {
        return None;
    };

    Some(comprehension.as_ref().clone())
}
