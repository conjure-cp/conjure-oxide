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
///
/// Algorithm sketch:
/// 1. Match one comprehension node.
/// 2. Build a temporary generator submodel from its qualifiers/guards.
/// 3. Materialise quantified declarations as temporary `find` declarations.
/// 4. Wrap that submodel as a standalone temporary model, with search order restricted to the
///    quantified names.
/// 5. Rewrite the temporary model using the configured rewriter and Minion-oriented rules.
/// 6. Solve the rewritten temporary model with Minion and keep only quantified assignments from
///    each solution.
/// 7. Instantiate the original return expression under each quantified assignment.
/// 8. Replace the comprehension by a matrix literal containing all instantiated return values.
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
///
/// Algorithm sketch:
/// 1. Match an AC operator whose single child is a comprehension.
/// 2. Build a temporary generator submodel from the comprehension qualifiers/guards.
/// 3. Add a derived constraint from the return expression to this generator model:
///    localise non-local references, and replace non-quantified fragments with dummy variables so
///    the constraint depends only on locally solvable symbols.
/// 4. Materialise quantified declarations as temporary `find` declarations in the temporary model.
/// 5. Rewrite and solve the temporary model with Minion; keep only quantified assignments.
/// 6. Instantiate the original return expression under those assignments.
/// 7. Rebuild the same AC operator around the instantiated matrix literal.
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
