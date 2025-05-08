// //! Normalising rules for `Neq` and `Eq`.

// use conjure_core::ast::{Expression as Expr, SymbolTable, Typeable};
// use conjure_core::rule_engine::{
//     register_rule, ApplicationError::RuleNotApplicable, ApplicationResult, Reduction,
// };

// use conjure_core::ast::ReturnType::Set;
// use conjure_essence_macros::essence_expr;
// use Expr::*;

// /// Converts a negated `Neq` to an `Eq`
// ///
// /// ```text
// /// not(neq(x)) ~> eq(x)
// /// ```
// #[register_rule(("Base", 8800))]
// fn negated_neq_to_eq(expr: &Expr, _: &SymbolTable) -> ApplicationResult {
//     match expr {
//         Not(_, a) => match a.as_ref() {
//             Neq(_, b, c) if (b.is_safe() && c.is_safe()) => {
//                 Ok(Reduction::pure(Eq(Metadata::new(), b.clone(), c.clone())))
//             }
//             _ => Err(RuleNotApplicable),
//         },
//         _ => Err(RuleNotApplicable),
//     }
// }

// /// Converts a negated `Neq` to an `Eq`
// ///
// /// ```text
// /// not(eq(x)) ~> neq(x)
// /// ```
// /// don't want this to apply to sets
// #[register_rule(("Base", 8800))]
// fn negated_eq_to_neq(expr: &Expr, _: &SymbolTable) -> ApplicationResult {
//     match expr {
//         Not(_, a) => match a.as_ref() {
//             Eq(_, b, c) if (b.is_safe() && c.is_safe()) => {
//                 if let Some(Set(_)) = b.as_ref().return_type() {
//                     return Err(RuleNotApplicable);
//                 }
//                 if let Some(Set(_)) = c.as_ref().return_type() {
//                     return Err(RuleNotApplicable);
//                 }
//                 Ok(Reduction::pure(essence_expr!(&b != &c)))
//             }
//             _ => Err(RuleNotApplicable),
//         },
//         _ => Err(RuleNotApplicable),
//     }
// }
