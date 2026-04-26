use conjure_cp::ast::comprehension::{self, Comprehension, ComprehensionQualifier};
use conjure_cp::ast::{Atom, DeclarationPtr, Metadata};
use conjure_cp::ast::{Expression as Expr, Moo, SymbolTable};
use conjure_cp::into_matrix_expr;
use conjure_cp::rule_engine::{Reduction, Rule};
use conjure_cp::rule_engine::{
    ApplicationError::RuleNotApplicable, ApplicationResult, register_rule,
};
use tracing::instrument::WithSubscriber;
use uniplate::Biplate;

use Expression::{And, Eq, SubsetEq};
// -- a intersect b ~~> or([ x = i | i <- b , i in a])
// [return_expr | A intersect B, qualifiers] ~~> [return_expr | i <- A, i in B, qualifiers]
#[register_rule("Base", 8700, [Comprehension])]
fn intersect(expr: &Expr, _: &SymbolTable) -> ApplicationResult {
    match expr {
        Expr::Comprehension(_, comp) => {
            // iterate through qualifiers 
            for qualifier in &comp.qualifiers {
                if let ComprehensionQualifier::ExpressionGenerator { ptr } = qualifier {
                    // clone the pointer to the qualifier that is an expression generator 
                    let gen_decl = ptr.clone;
                    
                    // match on qualifiers of form A intersect B
                    // Assign clones of the sets to a and b
                    let Some((a, b)) = (match ptr.as_quantified_expr() {
                        Some(expr_guard) => match &*expr_guard {
                            Expr::Intersect(_, a, b) => Some((a.clone(), b.clone())),
                            _ => None,
                        },
                        None => None,
                    }) else {
                        continue;
                    };
                    
                    //QUESTION: what is the purpose of not including b_ptr and then including it in the second one

                    // [return_expr | i <- A, (i in B)...]
                    let (comprehension1, a_ptr) = 
                        rewrite_intersect(comp.as_ref(), &gen_decl, a.into());

                    // add condition (i in B)
                    comprehension1
                        .qualifiers
                        .push(ComprehensionQualifier::Condition((Expr::In(
                            Metadata::new(),
                            Moo::new(Expr::Atomic(Metadata::new(), Atom::new_ref(a_ptr))), 
                            b
                        ))));


                    // return comprehension
                    return Ok(Reduction::pure(Expr::Comprehension(Metadata::new(), comprehension1.into())))

                }
            }
            // if none of the qualifiers are of the form "intersect"
            Err(RuleNotApplicable)
        }
        // if expr does not match a comprehension then throw err
        _ => Err(Rule)
    }
}




fn rewrite_intersect (
    comp: &Comprehension,
    gen_decl: &DeclarationPtr,
    replacement_expr:Expr,
) -> (Comprehension, DeclarationPtr) {
    // returns a ptr to the added expression of the comprehension
    let replacement_ptr = 
        DeclarationPtr::new_quantified_expr(gen_decl.name().clone(), replacement_expr);
    let mut comprehension = comp.clone();

    comprehension.symbols() = symbols.detach();

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

    comprehension
            .symbols()
            .write()
            .update_insert(replacement_ptr.clone());
    
    // update qualifiers 
    for qualifier in &comprehension.qualifiers {
        match qualifier  {
            Comprehension::ExpressionGenerator { ptr}
            | ComprehensionQualifier::Generator { ptr } => {
                comprehension.sumbols.write().update_insert(ptr.clone());
            }
        }
    }
    
    // return
    (comprehension, replacement_ptr)
}