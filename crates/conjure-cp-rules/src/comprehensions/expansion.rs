//! Comprehension expansion rules

mod expand_native;
mod expand_via_solver;
mod expand_via_solver_ac;

pub use expand_native::expand_native;
pub use expand_via_solver::expand_via_solver;
pub use expand_via_solver_ac::expand_via_solver_ac;

use std::collections::VecDeque;

use conjure_cp::{
    ast::{
        Expression as Expr, SymbolTable,
        comprehension::{
            Comprehension, QuantifiedExpander, quantified_expander_for_comprehensions,
        },
    },
    into_matrix_expr,
    rule_engine::{
        ApplicationError::RuleNotApplicable, ApplicationResult, Reduction, register_rule,
    },
};
use uniplate::Biplate;

use uniplate::Uniplate;

use conjure_cp::rule_engine::register_rule_set;

// optimised comprehension expansion for associative-commutative operators
register_rule_set!("Better_AC_Comprehension_Expansion", ("Base"));

/// Expand compatible comprehensions using AC optimisations via the solver-backed path.
#[register_rule(("Better_AC_Comprehension_Expansion", 2001))]
fn expand_comprehension_ac(expr: &Expr, symbols: &SymbolTable) -> ApplicationResult {
    if quantified_expander_for_comprehensions() != QuantifiedExpander::ExpandViaSolverAc {
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

#[register_rule(("Base", 2000))]
fn expand_comprehension(expr: &Expr, symbols: &SymbolTable) -> ApplicationResult {
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
    let results = match quantified_expander_for_comprehensions() {
        QuantifiedExpander::ExpandNative => {
            expand_native(comprehension.as_ref().clone(), &mut symbols)
        }
        QuantifiedExpander::ExpandViaSolver | QuantifiedExpander::ExpandViaSolverAc => {
            expand_via_solver(comprehension.as_ref().clone(), &mut symbols)
        }
    }
    .or(Err(RuleNotApplicable))?;

    Ok(Reduction::with_symbols(into_matrix_expr!(results), symbols))
}
