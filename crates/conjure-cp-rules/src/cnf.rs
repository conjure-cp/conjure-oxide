/***********************************************************************************/
/*        This file contains rules for converting logic expressions to CNF         */
/***********************************************************************************/

use conjure_cp::rule_engine::register_rule_set;
use conjure_cp::solver::SolverFamily;

use conjure_cp::ast::Metadata;
use conjure_cp::ast::{Expression as Expr, Moo};
use conjure_cp::rule_engine::{
    ApplicationError::RuleNotApplicable, ApplicationResult, Reduction, register_rule,
};

use conjure_cp::ast::SymbolTable;
use conjure_cp::matrix_expr;

register_rule_set!("CNF", ("Base"), (SolverFamily::Sat));

/// Converts an implication to cnf
///
/// ```text
/// x -> y ~~> !x \/ y
/// ```
#[register_rule(("CNF", 4100))]
fn remove_implication(expr: &Expr, _: &SymbolTable) -> ApplicationResult {
    let Expr::Imply(_, _, _) = expr else {
        return Err(RuleNotApplicable);
    };

    // now that we know the rule applies, we can clone the expression.
    let Expr::Imply(_, x, y) = expr.clone() else {
        unreachable!()
    };

    Ok(Reduction::pure(Expr::Or(
        Metadata::new(),
        Moo::new(matrix_expr![
            Expr::Not(Metadata::new(), x),
            Moo::unwrap_or_clone(y)
        ]),
    )))
}

/// Converts an equivalence to cnf
///
/// ```text
/// x = y ~~> (x -> y) /\ (y -> x) ~~> (!x \/ y) /\ (!y \/ x)
///
/// This converts boolean expressions using equivalence to CNF.
/// ```
#[register_rule(("CNF", 4100))]
fn remove_equivalence(expr: &Expr, _: &SymbolTable) -> ApplicationResult {
    let Expr::Eq(_, _, _) = expr else {
        return Err(RuleNotApplicable);
    };

    // now that we know this rule applies, clone the expr
    let Expr::Eq(_, x, y) = expr.clone() else {
        return Err(RuleNotApplicable);
    };

    Ok(Reduction::pure(Expr::And(
        Metadata::new(),
        Moo::new(matrix_expr![
            Expr::Or(
                Metadata::new(),
                Moo::new(matrix_expr![
                    Expr::Not(Metadata::new(), x.clone()),
                    Moo::unwrap_or_clone(y.clone())
                ]),
            ),
            Expr::Or(
                Metadata::new(),
                Moo::new(matrix_expr![
                    Moo::unwrap_or_clone(x),
                    Expr::Not(Metadata::new(), y)
                ]),
            )
        ]),
    )))
}
