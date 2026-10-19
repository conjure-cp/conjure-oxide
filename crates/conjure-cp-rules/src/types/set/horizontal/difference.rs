use crate::shared::utils::replace_expression_generator_source;
use conjure_cp::ast::comprehension::ComprehensionQualifier;
use conjure_cp::ast::{Atom, Metadata};
use conjure_cp::ast::{Expression as Expr, Moo, SymbolTable};
use conjure_cp::rule_engine::RuleEffect;
use conjure_cp::rule_engine::{
    ApplicationError::RuleNotApplicable, ApplicationResult, register_rule,
};

// [ return_expr | i <- A - B ] ~~> [ return_expr | i <- A, !(i in B) ]
#[register_rule("Base", 8700, [Comprehension])]
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
                            Expr::Difference(_, a, b) => Some((a.clone(), b.clone())),
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

                    return Ok(RuleEffect::pure(Expr::Comprehension(
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

/// `x in (a - b)` ~~> `x in a /\ !(x in b)`.
///
/// Membership is where set difference has to be lowered once anything other than a comprehension
/// generator gets to it first -- an equality between sets, say, which expands to memberships.
#[register_rule("Base", 8700, [In])]
fn membership_in_difference(expr: &Expr, _: &SymbolTable) -> ApplicationResult {
    let Expr::In(_, member, collection) = expr else {
        return Err(RuleNotApplicable);
    };

    let Expr::Difference(_, a, b) = collection.as_ref() else {
        return Err(RuleNotApplicable);
    };

    let in_a = Expr::In(Metadata::new(), member.clone(), a.clone());
    let in_b = Expr::In(Metadata::new(), member.clone(), b.clone());

    Ok(RuleEffect::pure(Expr::And(
        Metadata::new(),
        Moo::new(conjure_cp::matrix_expr![
            in_a,
            Expr::Not(Metadata::new(), Moo::new(in_b))
        ]),
    )))
}
