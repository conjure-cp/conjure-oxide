use crate::ast::Expression;
use crate::rule::{Rule, RuleApplicationError, RuleApplicationResult, RuleKind};
use conjure_macros::*;
use inventory;
// pub fn rewrite_to_solver(model: Model, solver: Solver) -> Result<Model> {
//     todo!();
// }

// TODO: possibly a nice macro for this:
// #[rule(kind=Horizontal, name=this_rule)]
// fn this_rule(expr: Expression) -> Result<Expression> {...}
//
// rule.name=this_rule by default, but is overridable

//inventory::collect!(Rule);

//pub fn get_rules() -> Vec<Rule> {
//    inventory::iter::<Rule>().collect()
//}
//
//pub fn get_rules_by_kind(kind: RuleKind) -> Vec<Rule> {
//    inventory::iter::<Rule>().filter(|r| r.kind == kind).collect()
//}
