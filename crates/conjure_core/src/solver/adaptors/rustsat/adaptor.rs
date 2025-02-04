use std::any::type_name;
use std::fmt::format;
use std::iter::Inspect;
use std::ptr::null;
use std::vec;

use super::conversions::{self, conv_to_clause, conv_to_formula, instantiate_model_from_conjure};
use clap::error;
use minion_rs::ast::Model;
use rustsat::encodings::am1::Def;
use rustsat::solvers::{Solve, SolverResult};
use rustsat::types::Var as satVar;
use std::collections::HashMap;

use rustsat_minisat::core::Minisat;

use crate::ast::{Atom, Expression, Name};
use crate::metadata::Metadata;
use crate::solver::{
    self, private, SearchStatus, SolveSuccess, SolverAdaptor, SolverCallback, SolverError,
    SolverFamily, SolverMutCallback,
};
use crate::stats::SolverStats;
use crate::{ast as conjure_ast, model, Model as ConjureModel};

// use super::super::model_modifier::NotModifiable;
// use super::super::private;
// use super::super::SearchComplete::*;
// use super::super::SearchIncomplete::*;
// use super::super::SearchStatus::*;
// use super::super::SolverAdaptor;
// use super::super::SolverError;
// use super::super::SolverError::*;
// use super::super::SolverError::*;

use rustsat::instances::SatInstance;

use thiserror::Error;

/// A [SolverAdaptor] for interacting with the SatSolver generic and the types thereof.

pub struct SAT {
    __non_constructable: private::Internal,
    model_inst: Option<SatInstance>,
    var_map: Option<HashMap<i32, satVar>>,
    solver_inst: Minisat,
}

impl private::Sealed for SAT {}

impl Default for SAT {
    fn default() -> Self {
        SAT {
            __non_constructable: private::Internal,
            model_inst: None,
            var_map: None,
            solver_inst: Minisat::default(),
        }
    }
}

impl SAT {
    pub fn new(model: ConjureModel) -> Self {
        let model_to_use: Option<SatInstance> = Some(SatInstance::new());
        SAT {
            __non_constructable: private::Internal,
            model_inst: model_to_use,
            var_map: None,
            solver_inst: Minisat::default(),
        }
    }

    pub fn add_clause_to_mod(&self, clause_vec: Vec<i32>) -> () {}
}

impl SolverAdaptor for SAT {
    fn solve(
        &mut self,
        callback: SolverCallback,
        _: private::Internal,
    ) -> Result<SolveSuccess, SolverError> {
        Err(SolverError::OpNotImplemented(format!("not supp")))
    }

    fn solve_mut(
        &mut self,
        callback: SolverMutCallback,
        _: private::Internal,
    ) -> Result<SolveSuccess, SolverError> {
        Err(SolverError::OpNotSupported("solve_mut".to_owned()))
    }

    fn load_model(&mut self, model: ConjureModel, _: private::Internal) -> Result<(), SolverError> {
        let inst_res: Result<SatInstance, SolverError> = instantiate_model_from_conjure(model);
        self.model_inst = Some(inst_res.unwrap());
        Ok(())
    }

    fn get_family(&self) -> SolverFamily {
        SolverFamily::SAT
    }
}

#[cfg(test)]
mod tests {
    // outdated
    use super::*;
    use crate::ast::{Expression, Name};
    use crate::metadata::Metadata;
    use crate::solver::adaptors::rustsat::conversions::{handle_lit, CNFError};
    use crate::solver::{
        self, SearchStatus, SolveSuccess, SolverCallback, SolverFamily, SolverMutCallback,
    };
    use crate::stats::SolverStats;
    use crate::{ast as conjure_ast, model, Model as ConjureModel};

    // #[test]
    // fn test_handle_expr_unexpected_expression() {
    //     let expr = Expression::Not(
    //         Metadata::new(),
    //         Box::new(Expression::Atomic(Metadata::new(), Name::MachineName(1))),
    //     );
    //     let result = handle_expr(expr);
    //     assert!(matches!(result, Err(CNFError::UnexpectedExpression(_))));
    // }

    #[test]
    fn test_handle_lit_unexpected_expression_inside_not() {
        let expr = Expression::Not(
            Metadata::new(),
            Box::new(Expression::And(Metadata::new(), vec![])),
        );
        let result = handle_lit(expr);
        assert!(matches!(
            result,
            Err(CNFError::UnexpectedExpressionInsideNot(_))
        ));
    }

    #[test]
    fn test_handle_lit_unexpected_literal_expression() {
        let expr = Expression::And(Metadata::new(), vec![]);
        let result = handle_lit(expr);
        assert!(matches!(
            result,
            Err(CNFError::UnexpectedLiteralExpression(_))
        ));
    }

    // #[test]
    // fn test_handle_or_unexpected_expression_inside_or() {
    //     let expr = Expression::Or(
    //         Metadata::new(),
    //         vec![
    //             Expression::Atomic(Metadata::new(), Name::MachineName(1)),
    //             Expression::And(Metadata::new(), vec![]),
    //         ],
    //     );
    //     let result = handle_or(expr);
    //     assert!(matches!(
    //         result,
    //         Err(CNFError::UnexpectedExpressionInsideOr(_))
    //     ));
    // }

    // #[test]
    // fn test_handle_expr_success_badval() {
    //     let expr = Expression::And(
    //         Metadata::new(),
    //         vec![Expression::Or(
    //             Metadata::new(),
    //             vec![
    //                 Expression::Atomic(Metadata::new(), Name::MachineName(1)),
    //                 Expression::Atomic(Metadata::new(), Name::MachineName(2)),
    //             ],
    //         )],
    //     );
    //     let result = handle_expr(expr);
    //     assert!(result.is_ok());
    //     let cnf_result = result.unwrap();
    //     assert_eq!(cnf_result.len(), 1); // Check that we have one clause
    //     assert_eq!(cnf_result[0].len(), 2); // Check that the clause has two literals
    // }

    // #[test]
    // fn test_handle_expr_success_goodval() {
    //     let expr = Expression::And(
    //         Metadata::new(),
    //         vec![Expression::Or(
    //             Metadata::new(),
    //             vec![
    //                 Expression::Atomic(Metadata::new(), Name::MachineName(0)),
    //                 Expression::Atomic(Metadata::new(), Name::MachineName(0)),
    //             ],
    //         )],
    //     );
    //     let result = handle_expr(expr);
    //     assert!(result.is_ok());
    //     let cnf_result = result.unwrap();
    //     // Check number of clauses
    //     assert_eq!(cnf_result.len(), 1);

    //     // Check number of literals in clause
    //     assert_eq!(cnf_result[0].len(), 2);

    //     // check literals
    //     assert_eq!(cnf_result[0][0], 0);
    //     assert_eq!(cnf_result[0][1], 0);
    // }

    // #[test]
    // fn test_handle_lit() {
    //     let expr = Expression::Not(
    //         Metadata::new(),
    //         Box::new(Expression::Atomic(Metadata::new(), Name::MachineName(0))),
    //     );

    //     let result = handle_lit(expr);
    //     assert!(result.is_ok());
    //     let lit_result = result.unwrap();
    //     assert_eq!(lit_result, 1); // Check that we have one clause
    // }
}
