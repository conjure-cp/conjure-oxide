use std::cell::RefCell;
use std::rc::Rc;

use conjure_cp::ast::comprehension::{Comprehension, ComprehensionBuilder};
use conjure_cp::ast::{Atom, DeclarationKind, Metadata};
use conjure_cp::ast::{Expression as Expr, Moo, ReturnType, SymbolTable, Typeable};
use conjure_cp::rule_engine::Reduction;
use conjure_cp::rule_engine::{
    ApplicationError::RuleNotApplicable, ApplicationResult, register_rule,
};
use conjure_cp::{into_matrix_expr, matrix_expr};

// [ return_expr | i <- A union B, guards...] -> flatten([[ return_expr | i <- A, guards...], [ return_expr | i <- B, !(i in A), guards...]; int(1..2)])
#[register_rule(("Base", 8700))]
fn union_set(expr: &Expr, _: &SymbolTable) -> ApplicationResult {
    match expr {
        Expr::Comprehension(_, comp) => {
            // find if any of the generators are generating from expressions
            for (name, decl) in comp.generator_submodel.symbols().clone().into_iter_local() {
                if let DeclarationKind::ElementOf(gen_expr) = decl.kind() {
                    // match on expression being of form A union B
                    match gen_expr {
                        Expr::Union(_, a, b) => {
                            // original comprehension return expression
                            let return_expr = comp.return_expression();

                            // extract guards from original comprehension
                            let guards = comp.generator_submodel.constraints();

                            // create [ return_expr | i <- A, guards...] part
                            let mut comprehension1 =
                                ComprehensionBuilder::new(Rc::new(RefCell::new(comp.generator_submodel.symbols().clone())));
                            let i1 = comprehension1
                                .generator_symboltable()
                                .borrow_mut()
                                .gensym(&a);
                            comprehension1 = comprehension1.generator(i1);
                            comprehension1.guards = guards;

                            // create [ return_expr | i <- B, !(i in A), guards...] part
                            let mut comprehension2 =
                                ComprehensionBuilder::new(Rc::new(RefCell::new(comp.generator_submodel.symbols().clone())));
                            let i2 = comprehension2
                                .generator_symboltable()
                                .borrow_mut()
                                .gensym(&b);
                            comprehension2 = comprehension2.generator(i1);
                            comprehension2 = comprehension2.guard(Expr::Not(
                                Metadata::new(),
                                Moo::new(Expr::In(Metadata::new(), Expr::Atomic(Metadata::new(), Atom::new_ref(i2)), Moo::new(b.clone()))),
                            ));
                            comprehension1.guards = guards;

                            return Ok(Reduction::pure(Expr::Comprehension(
                                Metadata::new(),
                                Moo::new(Expr::Flatten(
                                    Metadata::new(),
                                    Moo::new(), // <-- don't know what do do about the fact flatten can take 1 or 2 arguments
                                    Moo::new(
                                        into_matrix_expr!(vec![comprehension1, comprehension2]; domain_int!(1..2)),
                                    ),
                                )),
                            )));
                        }
                        _ => Err(RuleNotApplicable),
                    }
                }
            }

            Err(RuleNotApplicable)
        }
        _ => Err(RuleNotApplicable),
    }
}
