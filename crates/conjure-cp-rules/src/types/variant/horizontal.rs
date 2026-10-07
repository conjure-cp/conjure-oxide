use conjure_cp::ast::{AbstractLiteral, Atom, Expression as Expr, SymbolTable};
use conjure_cp::rule_engine::{
    ApplicationError::RuleNotApplicable, ApplicationResult, RuleEffect as Reduction, register_rule,
};

/// Evaluate `active` when its variant operand is already a literal.
#[register_rule("ReprGeneral", 9800, [Active])]
fn variant_literal_active(expr: &Expr, _: &SymbolTable) -> ApplicationResult {
    let Expr::Active(_, subject, name) = expr else {
        return Err(RuleNotApplicable);
    };
    let active_name = match subject.as_ref() {
        Expr::AbstractLiteral(_, AbstractLiteral::Variant(field)) => &field.name,
        Expr::Atomic(
            _,
            Atom::Literal(conjure_cp::ast::Literal::AbstractLiteral(AbstractLiteral::Variant(
                field,
            ))),
        ) => &field.name,
        _ => return Err(RuleNotApplicable),
    };
    Ok(Reduction::pure(Expr::from(active_name == name)))
}
