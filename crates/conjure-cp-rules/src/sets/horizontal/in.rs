// #[register_rule(())]
//in rule


use conjure_cp::ast::Moo;
// Equals rule for sets
use conjure_cp::ast::Metadata;
use conjure_cp::ast::{Expression, ReturnType::Set, SymbolTable, Typeable};
use conjure_cp::matrix_expr;
use conjure_cp::rule_engine::Reduction;
use conjure_cp::rule_engine::{
    ApplicationError::RuleNotApplicable, ApplicationResult, register_rule,
};

use Expression::{And, Eq, SubsetEq};
// -- a in b ~~> or([ a = i | i <- b ])
// -- a in b ~~> or([ a = i | i in b ])
//this is written assuming the 1199 pr is implemented 
// and also that there is a way to generate comprehension variables from an expression 
#[register_rule(("Base", 9000))] //tbd, might change lol
fn rule_In(expr: &Expression, symbol: &SymbolTable) -> ApplicationResult {
    match expr {
        Expr::In(_, a, b) => {
            if let Some(ReturnType::Set(_)) = b.as_ref().return_type() {
                let mut comprehension = ComprehensionBuilder::new(Rc::new(
                    RefCell::new(symbol.clone()),
                ));
                let i = comprehension.generator_symboltable().borrow_mut().gensym(&b);
                comprehension = comprehension.generator(i); //do we need to pass a reference of i?
                let expr1 = Expr::Eq(Metadata::new(), Moo::new(a.clone()), Moo::new(i.clone()));
                Ok(Reduction::pure(Expr::Or(
                    Metadata::new(),
                    Moo::new(Expr::Comprehension(
                        Metadata::new(),
                        Moo::new(comprehension.with_return_value(
                            expr1,
                            Some(ACOperatorKind::Or),
                        )),
                    )),
                )))
            } else {
                Err(RuleNotApplicable)
            }
        }
        => Err(RuleNotApplicable),
    }
}
