// #[register_rule(())]
//in rule


use conjure_cp::ast::Moo;
// Equals rule for sets
use conjure_cp::ast::Metadata;
use conjure_cp::ast::abstract_comprehension::AbstractComprehensionBuilder;
use conjure_cp::ast::{Expression, ReturnType::Set, SymbolTable, Typeable};
use conjure_cp::matrix_expr;
use conjure_cp::rule_engine::Reduction;
use conjure_cp::rule_engine::{
    ApplicationError::RuleNotApplicable, ApplicationResult, register_rule,
};

use Expression::{And, Eq, SubsetEq};
// -- a intersect b ~~> or([ x = i | i <- b , i in a])
#[register_rule(("Base", 9000))]
fn intersect(expr: &Expression, symbol: &SymbolTable) -> ApplicationResult {
    match expr {
        Expr::Intersect(_, a, b) => {
            if let Some(ReturnType::Set(_)) = b.as_ref().return_type() {
                let mut comprehension = AbstractComprehensionBuilder::new(Rc::new(
                    RefCell::new(symbol.clone()),
                ));
                let i = comprehension.new_expression_generator(b);
                let expr1 = Expr::Eq(Metadata::new(), Moo::new(a.clone()), Moo::new(i.clone()));
                let expr2 = Expr::In(Metadata::new(), Moo::new(i.clone), Moo::new(a.clone()));
                comprehension = comprehension.add_condition(expr2);
                Ok(Reduction::pure(Expr::Or(
                    Metadata::new(),
                    Moo::new(Expr::AbstractComprehension(
                        Metadata::new(),
                        Moo::new(comprehension.with_return_value(
                            expr1
                        )),
                    )),
                )))
            } else {
                Err(RuleNotApplicable)
            }
        }
    }
}
