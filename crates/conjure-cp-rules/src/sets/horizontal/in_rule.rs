
use conjure_cp::ast::comprehension::{Comprehension, ComprehensionQualifier};
use conjure_cp::ast::{Atom, DeclarationPtr, Metadata};
use conjure_cp::ast::{Expression as Expr, Moo, SymbolTable};
use conjure_cp::into_matrix_expr;
use conjure_cp::rule_engine::Reduction;
use conjure_cp::rule_engine::{
    ApplicationError::RuleNotApplicable, ApplicationResult, register_rule,
};
use uniplate::Biplate;

// use Expression::{And, Eq, SubsetEq};
// -- [return_expr | a in b, qualifiers....] ~~> [return_expr | ]

// or([ a = i | i <- b ]) ... wait can;t this be rewritten as [ret_expr | i for i in a if i in b]
// and also that there is a way to generate comprehension variables from an expression 
#[register_rule(("Base", 9000))]
fn rule_in_set(expr: &Expr, _: &SymbolTable) -> ApplicationResult {
    // match expr onto a comprehension else fail
    match expr {
        Expr::Comprehension(_, compr) => {
            // need to sort oout how you want to extract the data and where to put it,
            // prob need to create a new comprehension that 
        }
        _ => Err(RuleNotApplicable)
    }
}

// fn rule_in(expr: &Expression, symbol: &SymbolTable) -> ApplicationResult {
//     match expr {
//         Expr::In(_, a, b) => {
//             if let Some(ReturnType::Set(_)) = b.as_ref().return_type() {
//                 let mut comprehension = AbstractComprehensionBuilder::new(Rc::new(
//                     RefCell::new(symbol.clone()),
//                 ));
//                 let i = comprehension.new_expression_generator(b);
//                 let expr1 = Expr::Eq(Metadata::new(), Moo::new(a.clone()), Moo::new(i.clone()));
//                 Ok(Reduction::pure(Expr::Or(
//                     Metadata::new(),
//                     Moo::new(Expr::AbstractComprehension(
//                         Metadata::new(),
//                         Moo::new(comprehension.with_return_value(
//                             expr1
//                         )),
//                     )),
//                 )))
//             } else {
//                 Err(RuleNotApplicable)
//             }
//         }
//         => Err(RuleNotApplicable),
//     }
// }



// use conjure_cp::ast::Moo;
// // Equals rule for sets
// use conjure_cp::ast::Metadata;
// use conjure_cp::ast::abstract_comprehension::AbstractComprehensionBuilder;
// use conjure_cp::ast::{Expression, ReturnType::Set, SymbolTable, Typeable};
// use conjure_cp::matrix_expr;
// use conjure_cp::rule_engine::Reduction;
// use conjure_cp::rule_engine::{
//     ApplicationError::RuleNotApplicable, ApplicationResult, register_rule,
// };
