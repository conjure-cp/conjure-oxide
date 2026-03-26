use conjure_cp::ast::comprehension::ComprehensionQualifier;
use conjure_cp::ast::{Atom, DeclarationPtr, Metadata};
use conjure_cp::ast::{Expression as Expr, Moo, SymbolTable};
use conjure_cp::into_matrix_expr;
use conjure_cp::rule_engine::Reduction;
use conjure_cp::rule_engine::{
    ApplicationError::RuleNotApplicable, ApplicationResult, register_rule,
};
use uniplate::Biplate;

// [ return_expr | i <- A union B, qualifiers...] -> flatten([[ return_expr | i <- A, qualifiers...], [ return_expr | i <- B, !(i in A), qualifiers...]; int(1..2)])
#[register_rule(("Base", 8700))]
fn union_set(expr: &Expr, _: &SymbolTable) -> ApplicationResult {
    match expr {
        Expr::Comprehension(_, comp) => {
            // find if any of the generators are generating from expressions
            for (i, qualifier) in comp.qualifiers.iter().enumerate() {
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
                    let mut comprehension1 = comp.clone();
                    // modify the generator expression in place to be A
                    if let Some(qual) = comprehension1.qualifiers.get_mut(i)
                        && let ComprehensionQualifier::ExpressionGenerator { ptr } = qual
                        && let Some(mut expr) = ptr.as_quantified_expr_mut()
                    {
                        *expr = a.clone().into();
                    } else {
                        panic!(
                            "union_set rule could not find ExpressionGenerator expr while trying to modify first comp in place to a"
                        );
                    }

                    // [ return_expr | i <- B, !(i in A), guards...] part
                    let mut comprehension2 = comp.clone();
                    // modify the generator to be just from b
                    let b_ptr =  DeclarationPtr::new_quantified_expr(gen_decl.name().clone(), b.into());
                    let b_gen = ComprehensionQualifier::ExpressionGenerator { ptr: b_ptr.clone() };
                    if let Some(qual2) = comprehension2.qualifiers.get_mut(i) {
                        *qual2 = b_gen;
                    } 

                    // replace all occurences of old generator in comprehension2 with new pointer
                    comprehension2.return_expression.transform_bi(&|atom: Atom| match atom {
                        Atom::Reference(reference) => {
                            if reference.clone().into_ptr() == gen_decl {
                                Atom::new_ref(b_ptr.clone())
                            } else {
                                Atom::Reference(reference)
                            }
                        }
                        other => other,
                    });

                    // add the condition !(i in A)
                    comprehension2
                        .qualifiers
                        .push(ComprehensionQualifier::Condition(Expr::Not(
                            Metadata::new(),
                            Moo::new(Expr::In(
                                Metadata::new(),
                                Moo::new(Expr::Atomic(Metadata::new(), Atom::new_ref(b_ptr))),
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

            Err(RuleNotApplicable)
        }
        _ => Err(RuleNotApplicable),
    }
}
