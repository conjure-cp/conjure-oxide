use conjure_cp::ast::Metadata;
use conjure_cp::ast::{Expression as Expr, Moo, ReturnType, SymbolTable, Typeable};
use conjure_cp::matrix_expr;
use conjure_cp::rule_engine::Reduction;
use conjure_cp::rule_engine::{
    ApplicationError::RuleNotApplicable, ApplicationResult, register_rule,
};

// A in (B union C) -> A in B \/ A in C
#[register_rule(("Base", 8700))]
fn union_set(expr: &Expr, _: &SymbolTable) -> ApplicationResult {
    match expr {
        Expr::In(_, a, expr2) => {
            match &**expr2 {
                Expr::Union(_, b, c) => {
                    if let Some(ReturnType::Set(_)) = b.as_ref().return_type() {
                        if let Some(ReturnType::Set(_)) = c.as_ref().return_type() {
                            let expr1 = Expr::In(Metadata::new(), a.clone(), b.clone());
                            let expr2 = Expr::In(Metadata::new(), a.clone(), c.clone());
                            Ok(Reduction::pure(Expr::Or(
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
        }
        _ => Err(RuleNotApplicable),
    }
}
