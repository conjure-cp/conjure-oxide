// Equals rule for sets
use conjure_core::ast::{AbstractLiteral, Atom, Expression, SymbolTable};
use conjure_core::matrix_expr;
use conjure_core::metadata::Metadata;
use conjure_core::rule_engine::Reduction;
use conjure_core::rule_engine::{
    register_rule, ApplicationError::RuleNotApplicable, ApplicationResult,
};

use Expression::*;

#[register_rule(("Base", 8800))]
fn eq_to_subset_eq(expr: &Expression, _: &SymbolTable) -> ApplicationResult {
    match expr {
        Eq(_, lhs, rhs) => match lhs.as_ref() {
            Atomic(m1, Atom::Reference(a1)) => {
                let expr1 = Expression::Atomic(m1.clone(), Atom::Reference(a1.clone()));
                match rhs.as_ref() {
                    Atomic(m2, Atom::Reference(a2)) => {
                        let expr2 = Expression::Atomic(m2.clone(), Atom::Reference(a2.clone()));
                        let expr3 = SubsetEq(
                            Metadata::new(),
                            Box::new(expr1.clone()),
                            Box::new(expr2.clone()),
                        );
                        let expr4 = SubsetEq(
                            Metadata::new(),
                            Box::new(expr2.clone()),
                            Box::new(expr1.clone()),
                        );

                        return Ok(Reduction::pure(And(
                            Metadata::new(),
                            Box::new(matrix_expr![expr3.clone(), expr4.clone()]),
                        )));
                    }
                    AbstractLiteral(m2, AbstractLiteral::Set(a2)) => {
                        let expr2 = Expression::AbstractLiteral(
                            m2.clone(),
                            AbstractLiteral::Set(a2.clone()),
                        );
                        let expr3 = SubsetEq(
                            Metadata::new(),
                            Box::new(expr1.clone()),
                            Box::new(expr2.clone()),
                        );
                        let expr4 = SubsetEq(
                            Metadata::new(),
                            Box::new(expr2.clone()),
                            Box::new(expr1.clone()),
                        );

                        return Ok(Reduction::pure(And(
                            Metadata::new(),
                            Box::new(matrix_expr![expr3.clone(), expr4.clone()]),
                        )));
                    }
                    _ => return Err(RuleNotApplicable),
                }
            }
            AbstractLiteral(m1, AbstractLiteral::Set(a1)) => {
                let expr1 =
                    Expression::AbstractLiteral(m1.clone(), AbstractLiteral::Set(a1.clone()));
                match rhs.as_ref() {
                    Atomic(m2, Atom::Reference(a2)) => {
                        let expr2 = Expression::Atomic(m2.clone(), Atom::Reference(a2.clone()));
                        let expr3 = SubsetEq(
                            Metadata::new(),
                            Box::new(expr1.clone()),
                            Box::new(expr2.clone()),
                        );
                        let expr4 = SubsetEq(
                            Metadata::new(),
                            Box::new(expr2.clone()),
                            Box::new(expr1.clone()),
                        );

                        return Ok(Reduction::pure(And(
                            Metadata::new(),
                            Box::new(matrix_expr![expr3.clone(), expr4.clone()]),
                        )));
                    }
                    AbstractLiteral(m2, AbstractLiteral::Set(a2)) => {
                        let expr2 = Expression::AbstractLiteral(
                            m2.clone(),
                            AbstractLiteral::Set(a2.clone()),
                        );
                        let expr3 = SubsetEq(
                            Metadata::new(),
                            Box::new(expr1.clone()),
                            Box::new(expr2.clone()),
                        );
                        let expr4 = SubsetEq(
                            Metadata::new(),
                            Box::new(expr2.clone()),
                            Box::new(expr1.clone()),
                        );

                        return Ok(Reduction::pure(And(
                            Metadata::new(),
                            Box::new(matrix_expr![expr3.clone(), expr4.clone()]),
                        )));
                    }
                    _ => return Err(RuleNotApplicable),
                }
            }
            _ => return Err(RuleNotApplicable),
        },
        _ => Err(RuleNotApplicable),
    }
}
