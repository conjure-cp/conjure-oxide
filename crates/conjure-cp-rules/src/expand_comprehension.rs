use std::collections::VecDeque;

use conjure_cp_core::{
    ast::{Expression as Expr, SymbolTable, comprehension::Comprehension},
    into_matrix_expr,
    rule_engine::{
        ApplicationError::RuleNotApplicable, ApplicationResult, Reduction, register_rule,
    },
};
use uniplate::Biplate;

use uniplate::Uniplate;

use conjure_cp_rule_macros::register_rule_set;

// optimised comprehension expansion for associative-commutative operators
register_rule_set!("Better_AC_Comprehension_Expansion", ("Base"));

/// expand compatible comprehensions using ac optimisations / Comprehension::expand_ac.

#[register_rule(("Better_AC_Comprehension_Expansion", 2001))]
fn expand_comprehension_ac(expr: &Expr, symbols: &SymbolTable) -> ApplicationResult {
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
    let results = (**comprehension)
        .clone()
        .expand_ac(&mut symbols, ac_operator_kind)
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
    let results = comprehension
        .as_ref()
        .clone()
        .expand_simple(&mut symbols)
        .or(Err(RuleNotApplicable))?;

    Ok(Reduction::with_symbols(into_matrix_expr!(results), symbols))
}

// #[register_rule(("Base", 8000))]
// fn comprehension_move_static_guard_into_generators(
//     expr: &Expr,
//     _: &SymbolTable,
// ) -> ApplicationResult {
//     let Expr::Comprehension(_, comprehension) = expr else {
//         return Err(RuleNotApplicable);
//     };

//     let mut comprehension = comprehension.clone();
//     let Expr::Imply(_, guard, expr) = comprehension.clone().return_expression() else {
//         return Err(RuleNotApplicable);
//     };

//     if comprehension.add_induction_guard(*guard) {
//         comprehension.replace_return_expression(*expr);
//         Ok(Reduction::pure(Expr::Comprehension(
//             Metadata::new(),
//             comprehension,
//         )))
//     } else {
//         Err(RuleNotApplicable)
//     }
// }
