use conjure_cp::ast::abstract_comprehension::{
    ExpressionGenerator, Generator, Qualifier,
};
use conjure_cp::ast::{Atom, Metadata};
use conjure_cp::ast::{Expression as Expr, Moo, SymbolTable};
use conjure_cp::rule_engine::{
    ApplicationError::RuleNotApplicable, ApplicationResult, register_rule,
};
use conjure_cp::rule_engine::{Reduction};
use conjure_cp::{into_matrix_expr};

// [ return_expr | i <- A union B, qualifiers...] -> flatten([[ return_expr | i <- A, qualifiers...], [ return_expr | i <- B, !(i in A), qualifiers...]; int(1..2)])
#[register_rule(("Base", 8700))]
fn union_set(expr: &Expr, _: &SymbolTable) -> ApplicationResult {
    match expr {
        Expr::AbstractComprehension(_, comp) => {
            // find if any of the generators are generating from expressions
            for qualifier in &comp.qualifiers {
                if let Qualifier::Generator(Generator::ExpressionGenerator(ExpressionGenerator {
                    decl,
                    expression: gen_expr,
                })) = qualifier
                {
                    // match on expression being of form A union B
                    if let Expr::Union(_, a, b) = gen_expr {
                        // [ return_expr | i <- A, guards...] part
                        let mut comprehension1 = comp.clone();
                        // identify the generator qualifier again and modify the expression in place to be A
                        for qualifier in &mut comprehension1.qualifiers {
                            if let Qualifier::Generator(Generator::ExpressionGenerator(
                                ExpressionGenerator {
                                    decl: _,
                                    expression,
                                },
                            )) = qualifier
                            {
                                *expression = a.clone().into();
                            }
                        }

                        // [ return_expr | i <- B, !(i in A), guards...] part
                        let mut comprehension2 = comp.clone();
                        // identify the generator qualifier again and modify the expression in place to be B
                        for qualifier in &mut comprehension2.qualifiers {
                            if let Qualifier::Generator(Generator::ExpressionGenerator(
                                ExpressionGenerator {
                                    decl: _,
                                    expression,
                                },
                            )) = qualifier
                            {
                                *expression = b.clone().into();
                            }
                        }
                        // add the condition !(i in A)
                        comprehension2
                            .qualifiers
                            .push(Qualifier::Condition(Expr::Not(
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
                            Moo::new(
                                into_matrix_expr!(vec![Expr::AbstractComprehension(Metadata::new(), comprehension1), Expr::AbstractComprehension(Metadata::new(), comprehension2)]),
                            ),
                        )));
                    }
                }
            }

            Err(RuleNotApplicable)
        }
        _ => Err(RuleNotApplicable),
    }
}
