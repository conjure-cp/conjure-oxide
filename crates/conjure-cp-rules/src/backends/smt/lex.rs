use crate::shared::lex::{lex_elements_to_recursive_or, lex_operand_elements};
use conjure_cp::ast::{Expression as Expr, SymbolTable};
use conjure_cp::rule_engine::{
    ApplicationError::RuleNotApplicable, ApplicationResult, RuleEffect, register_rule,
};

/// Expand scalar lexicographic comparisons for library-backed solver relations.
#[register_rule("Smt", 2001, [LexLt, LexLeq])]
pub(crate) fn expand_lex_lt_leq(expr: &Expr, _: &SymbolTable) -> ApplicationResult {
    let (lhs, rhs) = match expr {
        Expr::LexLt(_, lhs, rhs) | Expr::LexLeq(_, lhs, rhs) => (lhs, rhs),
        _ => return Err(RuleNotApplicable),
    };

    let lhs_elements = lex_operand_elements(lhs)?;
    let rhs_elements = lex_operand_elements(rhs)?;
    let allow_equality = matches!(expr, Expr::LexLeq(..));

    Ok(RuleEffect::pure(lex_elements_to_recursive_or(
        &lhs_elements,
        &rhs_elements,
        allow_equality,
    )))
}

#[cfg(test)]
mod tests {
    use super::*;
    use conjure_cp::ast::{Metadata, Moo};
    use conjure_cp::matrix_expr;

    #[test]
    fn expands_explicit_lists_even_when_element_domains_are_unavailable() {
        let lhs = matrix_expr![
            Expr::Metavar(Metadata::new(), "a".into()),
            Expr::Metavar(Metadata::new(), "b".into())
        ];
        let rhs = matrix_expr![
            Expr::Metavar(Metadata::new(), "c".into()),
            Expr::Metavar(Metadata::new(), "d".into())
        ];
        assert!(lhs.domain_of().is_none());
        assert!(rhs.domain_of().is_none());

        let comparison = Expr::LexLeq(Metadata::new(), Moo::new(lhs), Moo::new(rhs));
        let result = expand_lex_lt_leq(&comparison, &SymbolTable::new())
            .expect("explicit list order is enough to expand lex");

        assert!(!matches!(result.new_expression, Expr::LexLeq(..)));
    }
}
