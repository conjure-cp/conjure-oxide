use conjure_cp::{ast::{Expression as Expr, Metadata, SymbolTable, comprehension::ComprehensionBuilder}, rule_engine::{ApplicationError::RuleNotApplicable, ApplicationResult, Reduction, Rule, register_rule}};

// A subsetEq B ~~> and([ i in A | i <- B ])
#[register_rule(("Base", 8700))]
fn subseteq_set(expr: &Expr, scope: &SymbolTable) -> ApplicationResult {
    match expr {
        Expr::SubsetEq(_, a, b) => {
            let comp_builder = ComprehensionBuilder::new(scope);


            Ok(Reduction::pure(
                Expr::And(Metadata::new(), comp)
            ))
        }
        _ => Err(RuleNotApplicable)
    }
}