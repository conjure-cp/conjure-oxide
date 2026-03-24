use conjure_cp::ast::comprehension::ComprehensionQualifier;
use conjure_cp::ast::{Atom, Metadata};
use conjure_cp::ast::{Expression as Expr, Moo, SymbolTable};
use conjure_cp::into_matrix_expr;
use conjure_cp::rule_engine::Reduction;
use conjure_cp::rule_engine::{
    ApplicationError::RuleNotApplicable, ApplicationResult, register_rule,
};

// [ return_expr | i <- A union B, qualifiers...] -> flatten([[ return_expr | i <- A, qualifiers...], [ return_expr | i <- B, !(i in A), qualifiers...]; int(1..2)])
#[register_rule(("Base", 8700))]
fn union_set(expr: &Expr, _: &SymbolTable) -> ApplicationResult {
    match expr {
        Expr::Comprehension(_, comp) => {
            // find if any of the generators are generating from expressions
            for (i, qualifier) in comp.qualifiers.iter().enumerate() {
                if let ComprehensionQualifier::ExpressionGenerator { ptr } = qualifier {
                    // match on expression being of form A union B
                    if let Expr::Union(_, a, b) = expr.into() {
                        // [ return_expr | i <- A, guards...] part
                        let mut comprehension1 = comp.clone();
                        // modify the generator expression in place to be A
                        match comprehension1.qualifiers.get_mut(i) {
                            Some(ComprehensionQualifier::ExpressionGenerator { ptr }) => {
                                *expr = a;
                            }
                            _ => panic!(),
                        }

                        // [ return_expr | i <- B, !(i in A), guards...] part
                        let mut comprehension2 = comp.clone();
                        // identify the generator qualifier again and modify the expression in place to be B
                        match comprehension2.qualifiers.get_mut(i) {
                            Some(ComprehensionQualifier::ExpressionGenerator { decl, expr }) => {
                                *expr = b;
                            }
                            _ => panic!(),
                        }

                        // add the condition !(i in A)
                        comprehension2
                            .qualifiers
                            .push(ComprehensionQualifier::Condition(Expr::Not(
                                Metadata::new(),
                                Moo::new(Expr::In(
                                    Metadata::new(),
                                    Moo::new(Expr::Atomic(
                                        Metadata::new(),
                                        Atom::new_ref(decl.clone()),
                                    )),
                                    a.clone(),
                                )),
                            )));

                        return Ok(Reduction::pure(Expr::Flatten(
                            Metadata::new(),
                            None,
                            Moo::new(into_matrix_expr!(vec![
                                Expr::Comprehension(Metadata::new(), comprehension1),
                                Expr::Comprehension(Metadata::new(), comprehension2)
                            ])),
                        )));
                    }
                }
            }

            Err(RuleNotApplicable)
        }
        _ => Err(RuleNotApplicable),
    }
}
