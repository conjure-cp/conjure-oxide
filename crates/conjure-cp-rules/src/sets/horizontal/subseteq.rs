use conjure_cp::{
    ast::{
        Atom, Expression as Expr, Metadata, Moo, SymbolTable, SymbolTablePtr,
        ac_operators::ACOperatorKind, comprehension::ComprehensionBuilder,
    },
    bug,
    rule_engine::{
        ApplicationError::RuleNotApplicable, ApplicationResult, Reduction, register_rule,
    },
};

// A subsetEq B ~~> and([ i in B | i <- A ])
#[register_rule("Base", 8700, [SubsetEq])]
fn subseteq_set(expr: &Expr, scope: &SymbolTable) -> ApplicationResult {
    match expr {
        Expr::SubsetEq(_, a, b) => {
            let scope_ptr = SymbolTablePtr::new();
            *scope_ptr.write() = scope.clone();
            let mut comp_builder = ComprehensionBuilder::new(scope_ptr);

            let quant_name = comp_builder
                .generator_symboltable()
                .write()
                .clone()
                .gen_sym();

            comp_builder = comp_builder.expression_generator(quant_name.clone(), a.clone().into());
            let Some(quant_ptr) = comp_builder
                .generator_symboltable()
                .read()
                .lookup_local(&quant_name)
            else {
                bug!("Could not find quantified variable name in subseteq rule symbol table")
            };

            let return_expr = Expr::In(
                Metadata::new(),
                Moo::new(Expr::Atomic(Metadata::new(), Atom::new_ref(quant_ptr))),
                b.clone(),
            );

            let comp = comp_builder.with_return_value(return_expr, Some(ACOperatorKind::And));

            Ok(Reduction::pure(Expr::And(
                Metadata::new(),
                Moo::new(Expr::Comprehension(Metadata::new(), Moo::new(comp))),
            )))
        }
        _ => Err(RuleNotApplicable),
    }
}
