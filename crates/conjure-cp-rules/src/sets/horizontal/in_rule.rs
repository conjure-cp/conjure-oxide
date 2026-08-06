
use std::thread::ScopedJoinHandle;

use conjure_cp::ast::ac_operators::ACOperatorKind;
use conjure_cp::ast::comprehension::{Comprehension, ComprehensionBuilder, ComprehensionQualifier};
use conjure_cp::ast::{Atom, DeclarationPtr, Metadata, SymbolTablePtr};
use conjure_cp::ast::{Expression as Expr, Moo, SymbolTable};
use conjure_cp::{bug, into_matrix_expr};
use conjure_cp::rule_engine::Reduction;
use conjure_cp::rule_engine::{
    ApplicationError::RuleNotApplicable, ApplicationResult, register_rule,
};
use uniplate::Biplate;

// use Expression::{And, Eq, SubsetEq};
// A in B ~~> or([ a = i | i <- b ])
#[register_rule("Base", 9000, [In])]
fn rule_in_set(expr: &Expr, scope: &SymbolTable) -> ApplicationResult {
    // match expr onto a comprehension else fail
    match expr {
        Expr::In(_, a, b) => {
            // copy scope to maintain the same symboltable
            let scope_ptr = SymbolTablePtr::new();
            *scope_ptr.write() = scope.clone();
            let mut comp_builder = ComprehensionBuilder::new(scope_ptr);

            //create a qualifier that generates from b

            // this creates an internal representation for i which is reserved as a local variable to generator form
            let quant_name = comp_builder
                .generator_symboltable()
                .write()
                .clone()
                .gen_sym();

            // this creates an expression generator with the temporary variable (i) and the set as the one you're generating from (b)
            comp_builder = comp_builder.expression_generator(quant_name.clone() , b.clone().into());

            // get a ptr to the quantifier
            let Some(quant_ptr) = comp_builder
                .generator_symboltable()
                .read()
                .lookup_local(&quant_name)
            else {
                bug!("there is no quantified variable ://")
            }; 

            // create a return expr a = i
            let return_expr = Expr::Eq(
                Metadata::new(),
                Moo::new(Expr::Atomic(Metadata::new(), Atom::new_ref(quant_ptr))),
                a.clone(),
            );

            let comp = comp_builder.with_return_value(return_expr, Some(ACOperatorKind::Or));

            Ok(Reduction::pure(Expr::Or(
                Metadata::new(),
                Moo::new(Expr::Comprehension(Metadata::new(), Moo::new(comp))),
            )))
        }
        _ => Err(RuleNotApplicable)
    }
}


















