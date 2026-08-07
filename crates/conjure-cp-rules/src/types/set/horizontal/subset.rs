// Subset rule for sets
use conjure_cp::ast::Metadata;
use conjure_cp::ast::{Expression as Expr, Moo, ReturnType, SymbolTable, Typeable};
use conjure_cp::matrix_expr;
use conjure_cp::rule_engine::RuleEffect;
use conjure_cp::rule_engine::{
    ApplicationError::RuleNotApplicable, ApplicationResult, register_rule,
};

#[register_rule("Base", 8700, [Subset])]
fn subset_to_subset_eq_neq(expr: &Expr, _: &SymbolTable) -> ApplicationResult {
    match expr {
        Expr::Subset(_, a, b)
            if matches!(a.as_ref().return_type(), ReturnType::Set(_))
                && matches!(b.as_ref().return_type(), ReturnType::Set(_)) =>
        {
            let expr1 = Expr::SubsetEq(Metadata::new(), a.clone(), b.clone());
            let expr2 = Expr::Neq(Metadata::new(), a.clone(), b.clone());
            Ok(RuleEffect::pure(Expr::And(
                Metadata::new(),
                Moo::new(matrix_expr![expr1, expr2]),
            )))
        }
        _ => Err(RuleNotApplicable),
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use conjure_cp::ast::{AbstractLiteral, Literal};

    #[test]
    fn strict_set_subset_becomes_inclusion_and_inequality() {
        let lhs = Expr::from(Literal::AbstractLiteral(AbstractLiteral::Set(vec![
            1.into(),
        ])));
        let rhs = Expr::from(Literal::AbstractLiteral(AbstractLiteral::Set(vec![
            1.into(),
            2.into(),
        ])));
        let subset = Expr::Subset(
            Metadata::new(),
            Moo::new(lhs.clone()),
            Moo::new(rhs.clone()),
        );

        let rewritten = subset_to_subset_eq_neq(&subset, &SymbolTable::new())
            .unwrap()
            .new_expression;

        assert_eq!(
            rewritten,
            Expr::And(
                Metadata::new(),
                Moo::new(matrix_expr![
                    Expr::SubsetEq(
                        Metadata::new(),
                        Moo::new(lhs.clone()),
                        Moo::new(rhs.clone()),
                    ),
                    Expr::Neq(Metadata::new(), Moo::new(lhs), Moo::new(rhs)),
                ]),
            )
        );
    }
}
