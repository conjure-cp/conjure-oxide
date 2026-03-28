use conjure_cp::ast::comprehension::{Comprehension, ComprehensionQualifier};
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
                    let (comprehension1, _) =
                        rewrite_union_branch(comp.as_ref(), &gen_decl, a.clone().into());

                    // [ return_expr | i <- B, !(i in A), guards...] part
                    let (mut comprehension2, b_ptr) =
                        rewrite_union_branch(comp.as_ref(), &gen_decl, b.into());

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

/// Clone one union branch into its own detached comprehension scope and rewrite all uses of the
/// original quantified declaration to a fresh branch-local expression generator.
fn rewrite_union_branch(
    comp: &Comprehension,
    gen_decl: &DeclarationPtr,
    replacement_expr: Expr,
) -> (Comprehension, DeclarationPtr) {
    let replacement_ptr =
        DeclarationPtr::new_quantified_expr(gen_decl.name().clone(), replacement_expr);
    let mut comprehension = comp.clone();

    // detach the scope so rewriting this branch does not mutate the original
    // comprehension through shared pointers
    comprehension.symbols = comprehension.symbols.detach();

    // rewrite all uses of the original quantified declaration to the branch-local
    // generator declaration
    comprehension.return_expression =
        comprehension
            .return_expression
            .transform_bi(&|decl: DeclarationPtr| {
                if decl == *gen_decl {
                    replacement_ptr.clone()
                } else {
                    decl
                }
            });

    comprehension.qualifiers = comprehension
        .qualifiers
        .into_iter()
        .map(|qualifier| {
            qualifier.transform_bi(&|decl: DeclarationPtr| {
                if decl == *gen_decl {
                    replacement_ptr.clone()
                } else {
                    decl
                }
            })
        })
        .collect();

    // keep the detached local scope in sync with the rewritten generator
    // declarations used by this branch
    comprehension
        .symbols
        .write()
        .update_insert(replacement_ptr.clone());
    for qualifier in &comprehension.qualifiers {
        match qualifier {
            ComprehensionQualifier::ExpressionGenerator { ptr }
            | ComprehensionQualifier::Generator { ptr } => {
                comprehension.symbols.write().update_insert(ptr.clone());
            }
            ComprehensionQualifier::Condition(_) => {}
        }
    }

    (comprehension, replacement_ptr)
}
