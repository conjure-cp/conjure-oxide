// rules for concatenations of subsetEq with intersect and union
// analogous rules for subset, supset and supsetEq are not needed because these are converted to subsetEq first.
use conjure_cp::ast::Metadata;
use conjure_cp::ast::{Expression as Expr, Moo, ReturnType, SymbolTable, Typeable};
use conjure_cp::matrix_expr;
use conjure_cp::rule_engine::Reduction;
use conjure_cp::rule_engine::{
    ApplicationError::RuleNotApplicable, ApplicationResult, register_rule,
};

// A subsetEq (B intersect C) -> A subsetEq B /\ A subsetEq C
#[register_rule(("Base", 8700))]
fn subseteq_intersect(expr: &Expr, _: &SymbolTable) -> ApplicationResult {
    match expr {
        Expr::SubsetEq(_, a, rhs) => {
            if let Some(ReturnType::Set(_)) = a.as_ref().return_type() {
                match &**rhs {
                    Expr::Intersect(_, b, c) => {
                        if let Some(ReturnType::Set(_)) = b.as_ref().return_type() {
                            if let Some(ReturnType::Set(_)) = c.as_ref().return_type() {
                                let expr1 = Expr::SubsetEq(Metadata::new(), a.clone(), b.clone());
                                let expr2 = Expr::SubsetEq(Metadata::new(), a.clone(), c.clone());
                                Ok(Reduction::pure(Expr::And(
                                    Metadata::new(),
                                    Moo::new(matrix_expr![expr1, expr2]),
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
fn union_subseteq(expr: &Expr, _: &SymbolTable) -> ApplicationResult {
    match expr {
        Expr::SubsetEq(_, lhs, c) => {
            if let Some(ReturnType::Set(_)) = c.as_ref().return_type() {
                match &**lhs {
                    Expr::Union(_, a, b) => {
                        if let Some(ReturnType::Set(_)) = a.as_ref().return_type() {
                            if let Some(ReturnType::Set(_)) = b.as_ref().return_type() {
                                let expr1 = Expr::SubsetEq(Metadata::new(), a.clone(), b.clone());
                                let expr2 = Expr::SubsetEq(Metadata::new(), a.clone(), c.clone());
                                Ok(Reduction::pure(Expr::And(
                                    Metadata::new(),
                                    Moo::new(matrix_expr![expr1, expr2]),
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
