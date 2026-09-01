use conjure_cp::ast::{Expression as Expr, Metadata, Moo, SymbolTable};
use conjure_cp::rule_engine::{
    ApplicationError::RuleNotApplicable, ApplicationResult, RuleEffect, register_rule,
    register_rule_set,
};
use conjure_cp::settings::SolverFamily;

// BOOLEAN SAT ENCODING RULES:

// One rule set covers all three integer encodings. Which encoding a variable gets is a
// representation choice made per declaration, so every encoding's rules have to be live at once;
// each declines when its operands are not in its own encoding.
register_rule_set!("SAT", ("Base"), |f: &SolverFamily| matches!(
    f,
    SolverFamily::Sat
));

/// An auxiliary declaration is an equality.
///
/// See the note on the SMT copy of this rule: only Minion has flat constraint forms to absorb an
/// auxiliary into, so every other backend asserts the equality instead.
#[register_rule("SAT", 2000, [AuxDeclaration])]
fn aux_declaration_is_equality(expr: &Expr, _: &SymbolTable) -> ApplicationResult {
    let Expr::AuxDeclaration(_, reference, value) = expr else {
        return Err(RuleNotApplicable);
    };
    Ok(RuleEffect::pure(Expr::Eq(
        Metadata::new(),
        Moo::new(reference.clone().into()),
        value.clone(),
    )))
}
