// // Equals rule for sets
// use conjure_core::ast::{Atom, DeclarationKind, Domain, Expression, Literal, SymbolTable};
// use conjure_core::metadata::Metadata;
// use conjure_core::rule_engine::{
//     register_rule, ApplicationError::RuleNotApplicable, ApplicationResult,
// };

// use std::rc::Rc;
// use Expression::*;

// use crate::ast::{Declaration, SetAttr};
// use crate::rule_engine::Reduction;

// #[register_rule(("Base", 8800))]
// fn eq_to_subsetEq(expr: &Expr, _: &SymbolTable) -> ApplicationResult {
//    match expr {
//        Eq(_, a, b) if (a.is_safe() && b.is_safe())=> {
//            Ok(Reduction::pure(And(Metadata::new(), Box<Vec<(SubsetEq(Metadata::new(), a.clone(), b.clone()), SubsetEq(Metadata::new(), b.clone(), a.clone()))>>)))
//        }
//        _ => Err(RuleNotApplicable),
//    }
// }