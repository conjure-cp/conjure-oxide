// rules for concatenations of subsetEq with intersect and union
// analogous rules for subset, supset and supsetEq are not needed because these are converted to subsetEq first.
use conjure_core::ast::{Expression, ReturnType::Set, SymbolTable, Typeable};
use conjure_core::matrix_expr;
use conjure_core::metadata::Metadata;
use conjure_core::rule_engine::Reduction;
use conjure_core::rule_engine::{
    register_rule, ApplicationError::RuleNotApplicable, ApplicationResult,
};

use Expression::*;

// A subsetEq (B intersect C) -> A subsetEq B /\ A subsetEq C
#[register_rule(("Base", 8700))]
fn subseteq_intersect(expr: &Expression, _: &SymbolTable) -> ApplicationResult {
    match expr {
        SubsetEq(_, a, rhs) => {
            if let Some(Set(_)) = a.as_ref().return_type() {
                match &**rhs {
                    Intersect(_, b, c) => {
                        if let Some(Set(_)) = b.as_ref().return_type() {
                            if let Some(Set(_)) = c.as_ref().return_type() {
                                let expr1 = SubsetEq(Metadata::new(), a.clone(), b.clone());
                                let expr2 = SubsetEq(Metadata::new(), a.clone(), c.clone());
                                Ok(Reduction::pure(And(
                                    Metadata::new(),
                                    Box::new(matrix_expr![expr1.clone(), expr2.clone()]),
                                )))
                            } else {
                                Err(RuleNotApplicable)
                            }
                        } else {
                            Err(RuleNotApplicable)
                        }
                    }
                    _ => Err(RuleNotApplicable),
                }
            } else {
                Err(RuleNotApplicable)
            }
        }
        _ => Err(RuleNotApplicable),
    }
}

// (A union B) subsetEq C -> A subsetEq C /\ B subsetEq C
#[register_rule(("Base", 8700))]
fn union_subseteq(expr: &Expression, _: &SymbolTable) -> ApplicationResult {
    match expr {
        SubsetEq(_, lhs, c) => {
            if let Some(Set(_)) = c.as_ref().return_type() {
                match &**lhs {
                    Union(_, a, b) => {
                        if let Some(Set(_)) = a.as_ref().return_type() {
                            if let Some(Set(_)) = b.as_ref().return_type() {
                                let expr1 = SubsetEq(Metadata::new(), a.clone(), b.clone());
                                let expr2 = SubsetEq(Metadata::new(), a.clone(), c.clone());
                                Ok(Reduction::pure(And(
                                    Metadata::new(),
                                    Box::new(matrix_expr![expr1.clone(), expr2.clone()]),
                                )))
                            } else {
                                Err(RuleNotApplicable)
                            }
                        } else {
                            Err(RuleNotApplicable)
                        }
                    }
                    _ => Err(RuleNotApplicable),
                }
            } else {
                Err(RuleNotApplicable)
            }
        }
        _ => Err(RuleNotApplicable),
    }
}
