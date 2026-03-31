use crate::utils::replace_expression_generator_source;
use conjure_cp::ast::comprehension::ComprehensionQualifier;
use conjure_cp::ast::{Atom, Metadata};
use conjure_cp::ast::{Expression as Expr, Moo, SymbolTable};
use conjure_cp::rule_engine::Reduction;
use conjure_cp::rule_engine::{
    ApplicationError::RuleNotApplicable, ApplicationResult, register_rule,
};

// [ return_expr | i <- A - B ] ~~> [ return_expr | i <- A, !(i in B) ]
#[register_rule(("Base", 8700))]
fn difference_set(expr: &Expr, _: &SymbolTable) -> ApplicationResult {
    match expr {
        Expr::Comprehension(_, comp) => {
            // find if any of the generators are generating from expressions
            for qualifier in &comp.qualifiers {
                if let ComprehensionQualifier::ExpressionGenerator { ptr } = qualifier {
                    let gen_decl = ptr.clone();

                    // match on expression being of form A - B
                    let Some((a, b)) = (match ptr.as_quantified_expr() {
                        Some(expr_guard) => match &*expr_guard {
                            Expr::Minus(_, a, b) => Some((a.clone(), b.clone())),
                            _ => None,
                        },
                        None => None,
                    }) else {
                        continue;
                    };

                    // [ return_expr | i <- A, !(i in B), guards...]
                    let (mut comprehension, a_ptr) =
                        replace_expression_generator_source(comp.as_ref(), &gen_decl, a.into());

                    // add the condition !(i in B)
                    comprehension
                        .qualifiers
                        .push(ComprehensionQualifier::Condition(Expr::Not(
                            Metadata::new(),
                            Moo::new(Expr::In(
                                Metadata::new(),
                                Moo::new(Expr::Atomic(Metadata::new(), Atom::new_ref(a_ptr))),
                                b,
                            )),
                        )));

                    return Ok(Reduction::pure(Expr::Comprehension(
                        Metadata::new(),
                        comprehension.into(),
                    )));
                }
            }

            Err(RuleNotApplicable)
        }
        _ => Err(RuleNotApplicable),
    }
}
