/***********************************************************************************/
/*        This file contains rules for converting logic expressions to CNF         */
/***********************************************************************************/

use conjure_core::rule_engine::register_rule_set;
use conjure_core::solver::SolverFamily;

use conjure_core::ast::Expression as Expr;
use conjure_core::metadata::Metadata;
use conjure_core::rule_engine::{
    register_rule, ApplicationError, ApplicationError::RuleNotApplicable, ApplicationResult,
    Reduction,
};

use Expr::*;

use crate::ast::{Atom, SymbolTable};
use crate::{into_matrix_expr, matrix_expr};

register_rule_set!("CNF", ("Base"), (SolverFamily::SAT));

/// Converts an implication to cnf
///
/// ```text
/// x -> y ~> !x \/ y
/// ```
#[register_rule(("CNF", 4100))]
fn remove_implication(expr: &Expr, _: &SymbolTable) -> ApplicationResult {
    let Expr::Imply(_, x, y) = expr else {
        return Err(RuleNotApplicable);
    };

    Ok(Reduction::pure(Expr::Or(
        Metadata::new(),
        Box::new(matrix_expr![
            Expr::Not(Metadata::new(), x.clone()),
            *y.clone()
        ]),
    )))
}
