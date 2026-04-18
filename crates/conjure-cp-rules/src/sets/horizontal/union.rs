use conjure_cp::ast::comprehension::ComprehensionQualifier;
use conjure_cp::ast::{Atom, Metadata};
use conjure_cp::ast::{Expression as Expr, Moo, SymbolTable};
use conjure_cp::into_matrix_expr;
use conjure_cp::rule_engine::Reduction;
use conjure_cp::rule_engine::{
    ApplicationError::RuleNotApplicable, ApplicationResult, register_rule,
};

use crate::utils::replace_expression_generator_source;

// [ return_expr | i <- A union B, qualifiers...] -> flatten([[ return_expr | i <- A, qualifiers...], [ return_expr | i <- B, !(i in A), qualifiers...]; int(1..2)])
#[register_rule("Base", 8700, [Comprehension])]
fn union_set(expr: &Expr, _: &SymbolTable) -> ApplicationResult {
    match expr {
        Expr::Comprehension(_, comp) => {
            // find if any of the generators are generating from expressions
            for qualifier in &comp.qualifiers {
                if let ComprehensionQualifier::ExpressionGenerator { ptr } = qualifier {
                    let gen_decl = ptr.clone();

                    // match on expression being of form A union B
                    let Some((a, b)) = (match ptr.as_quantified_expr() {
                        Some(expr_guard) => match &*expr_guard {
                            Expr::Union(_, a, b) => Some((a.clone(), b.clone())),
                            _ => None,
                        },
                        None => None,
                    }) else {
                        continue;
                    };

                    // [ return_expr | i <- A, guards...] part
                    let (comprehension1, _) = replace_expression_generator_source(
                        comp.as_ref(),
                        &gen_decl,
                        a.clone().into(),
                    );

                    // [ return_expr | i <- B, !(i in A), guards...] part
                    let (mut comprehension2, b_ptr) =
                        replace_expression_generator_source(comp.as_ref(), &gen_decl, b.into());

                    // add the condition !(i in A)
                    comprehension2
                        .qualifiers
                        .push(ComprehensionQualifier::Condition(Expr::Not(
                            Metadata::new(),
                            Moo::new(Expr::In(
                                Metadata::new(),
                                Moo::new(Expr::Atomic(Metadata::new(), Atom::new_ref(b_ptr))),
                                a,
                            )),
                        )));

                    return Ok(Reduction::pure(Expr::Flatten(
                        Metadata::new(),
                        None,
                        Moo::new(into_matrix_expr!(vec![
                            Expr::Comprehension(Metadata::new(), comprehension1.into()),
                            Expr::Comprehension(Metadata::new(), comprehension2.into())
                        ])),
                    )));
                }
            }

            Err(RuleNotApplicable)
        }
        _ => Err(RuleNotApplicable),
    }
}
