/*****************************************************************************************************/
/*        This file contains rules for converting integer expressions to logical expressions         */
/*****************************************************************************************************/

use conjure_core::rule_engine::register_rule_set;
use conjure_core::solver::SolverFamily;

use conjure_core::ast::Expression as Expr;
use conjure_core::metadata::Metadata;
use conjure_core::rule_engine::{
    register_rule, ApplicationError::RuleNotApplicable, ApplicationResult, Reduction,
};

use Expr::*;

use crate::ast::SymbolTable;
use crate::matrix_expr;

register_rule_set!("IntBool", ("Base", "CNF"), (SolverFamily::SAT));
