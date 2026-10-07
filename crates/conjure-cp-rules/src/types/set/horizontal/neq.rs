// Supset rule for sets
use conjure_cp::ast::Metadata;
use conjure_cp::ast::{Expression as Expr, Moo, ReturnType, SymbolTable, Typeable};
use conjure_cp::rule_engine::RuleEffect;
use conjure_cp::rule_engine::{
    ApplicationError::RuleNotApplicable, ApplicationResult, register_rule,
};

#[register_rule("Base", 8700, [Neq])]
fn neq_not_eq_sets(expr: &Expr, _: &SymbolTable) -> ApplicationResult {
    match expr {
        Expr::Neq(_, a, b)
            if matches!(a.as_ref().return_type(), ReturnType::Set(_))
                && matches!(b.as_ref().return_type(), ReturnType::Set(_)) =>
        {
            Ok(RuleEffect::pure(Expr::Not(
                Metadata::new(),
                Moo::new(Expr::Eq(Metadata::new(), b.clone(), a.clone())),
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
    fn set_inequality_becomes_negated_equality() {
        let lhs = Expr::from(Literal::AbstractLiteral(AbstractLiteral::Set(vec![
            1.into(),
        ])));
        let rhs = Expr::from(Literal::AbstractLiteral(AbstractLiteral::Set(vec![
            2.into(),
        ])));
        let inequality = Expr::Neq(
            Metadata::new(),
            Moo::new(lhs.clone()),
            Moo::new(rhs.clone()),
        );

        let rewritten = neq_not_eq_sets(&inequality, &SymbolTable::new())
            .unwrap()
            .new_expression;

        assert_eq!(
            rewritten,
            Expr::Not(
                Metadata::new(),
                Moo::new(Expr::Eq(Metadata::new(), Moo::new(rhs), Moo::new(lhs),)),
            )
        );
    }
}
