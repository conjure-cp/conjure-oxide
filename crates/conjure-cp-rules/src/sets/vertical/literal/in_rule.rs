use conjure_cp::ast::Metadata;
use conjure_cp::rule_engine::Reduction;

use conjure_cp::ast::AbstractLiteral;
use conjure_cp::ast::Atom;
use conjure_cp::ast::Expression as Expr;
use conjure_cp::ast::Literal;

use conjure_cp::ast::SymbolTable;
use conjure_cp::rule_engine::{
    ApplicationError::RuleNotApplicable, ApplicationResult, register_rule,
};

// turns an in expression into a w-inset expression where x is an integer decision variable
// and the set is a set of integers like:
// x in {1,2,3} => w-inset(x, [1,2,3])
#[register_rule(("Minion", 1))]
fn in_set(expr: &Expr, _: &SymbolTable) -> ApplicationResult {
    match expr {
        Expr::In(_, a, b) => {
            let Expr::Atomic(_, Atom::Literal(Literal::AbstractLiteral(AbstractLiteral::Set(c)))) =
                b.as_ref()
            else {
                return Err(RuleNotApplicable);
            };

            let literals = c
                .iter()
                .map(i32::try_from)
                .collect::<Result<Vec<_>, _>>()
                .map_err(|_| RuleNotApplicable)?;

            if let Expr::Atomic(_, a) = a.as_ref() {
                Ok(Reduction::pure(Expr::MinionWInSet(
                    Metadata::new(),
                    a.clone(),
                    literals,
                )))
            } else {
                Err(RuleNotApplicable)
            }
        }
        _ => Err(RuleNotApplicable),
    }
}
