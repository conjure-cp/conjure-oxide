// #[register_rule(("ReprGeneral", 2000))]
// fn set_explicit_cardinality(expr: &Expression, _: &SymbolTable) -> ApplicationResult {
//     todo!()
// }
//
// #[register_rule(("ReprGeneral", 2000))]
// fn set_explicit_in(expr: &Expression, symtab: &SymbolTable) -> ApplicationResult {
//     guard!(
//         let Expression::In(_, lhs, rhs) = expr                        &&
//         let Expression::Atomic(_, Atom::Reference(re)) = rhs.as_ref() &&
//         let Some(repr) = re.get_repr_as::<SetExplicitWithSize>()
//         else {
//             return Err(RuleNotApplicable);
//         }
//     );
//
//     todo!()
// }
