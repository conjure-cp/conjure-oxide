/***********************************************************************************/
/*        This file contains rules for converting logic expressions to CNF         */
/***********************************************************************************/

use conjure_core::ast::Expression as Expr;
use conjure_core::rule_engine::{
    register_rule, register_rule_set, ApplicationError, ApplicationResult, Reduction,
};
use conjure_core::solver::SolverFamily;
use conjure_core::Model;

register_rule_set!("CNF", 100, ("Base"), (SolverFamily::SAT));
