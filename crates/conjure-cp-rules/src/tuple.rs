use crate::guard;
use crate::representation::tuple_to_atom::TupleToAtom;
use conjure_cp::ast::{Atom, Expression as Expr, Literal, Metadata, Reference, SymbolTable};
use conjure_cp::bug_assert_eq;
use conjure_cp::rule_engine::ApplicationError::RuleNotApplicable;
use conjure_cp::rule_engine::{ApplicationResult, Reduction, register_rule};
use conjure_cp::{bug_assert, essence_expr};

#[register_rule(("ReprGeneral", 2000))]
fn tuple_to_atom_index_lit(expr: &Expr, _: &SymbolTable) -> ApplicationResult {
    guard!(
        let Expr::SafeIndex(_, subject, indices) = expr        &&
        let Expr::Atomic(_, Atom::Reference(re)) = &**subject  &&
        let Some(Expr::Atomic(_, idx)) = indices.first()       &&
        let Atom::Literal(Literal::Int(idx)) = idx             &&
        let Some(repr) = re.get_repr_as::<TupleToAtom>()
        else {
            return Err(RuleNotApplicable);
        }
    );
    assert_eq!(indices.len(), 1, "tuple indexing is always one dimensional");

    let lhs = Reference::new(repr.elems[*idx as usize].clone());
    let rhs = &indices[1..];

    if rhs.is_empty() {
        Ok(Reduction::pure(lhs.into()))
    } else {
        let new_expr = Expr::SafeIndex(Metadata::new(), lhs.into(), Vec::from(rhs));
        Ok(Reduction::pure(new_expr))
    }
}

#[register_rule(("Bubble", 8000))]
fn tuple_index_to_bubble(expr: &Expr, _: &SymbolTable) -> ApplicationResult {
    guard!(
        let Expr::UnsafeIndex(_, subject, indices) = expr &&
        let Some(idx) = indices.first()                   &&
        let Some(idx_dom) = idx.domain_of()               &&
        let Some(dom) = subject.domain_of()               &&
        let Some(inner_doms) = dom.as_tuple()
        else {
            return Err(RuleNotApplicable);
        }
    );
    bug_assert_eq!(indices.len(), 1, "tuple indexing is always one dimensional");
    bug_assert!(
        idx_dom.as_int().is_some(),
        "tuple indexing expression must be integer"
    );

    let len = inner_doms.len() as i32;
    let bubble_cond = essence_expr!(r"(&idx >= 1) /\ (&idx <= &len)");
    let bubble_expr = Expr::SafeIndex(Metadata::new(), subject.clone(), indices.clone());

    let new_expr = Expr::Bubble(Metadata::new(), bubble_expr.into(), bubble_cond.into());
    Ok(Reduction::pure(new_expr))
}

// // convert equality to tuple equality
// #[register_rule(("Base", 2000))]
// fn tuple_equality(expr: &Expr, _: &SymbolTable) -> ApplicationResult {
//     let Expr::Eq(_, left, right) = expr else {
//         return Err(RuleNotApplicable);
//     };
//
//     let Expr::Atomic(_, Atom::Reference(decl)) = &**left else {
//         return Err(RuleNotApplicable);
//     };
//
//     let Name::WithRepresentation(_, reprs) = &decl.name() as &Name else {
//         return Err(RuleNotApplicable);
//     };
//
//     let Expr::Atomic(_, Atom::Reference(decl2)) = &**right else {
//         return Err(RuleNotApplicable);
//     };
//
//     let Name::WithRepresentation(_, reprs2) = &decl2.name() as &Name else {
//         return Err(RuleNotApplicable);
//     };
//
//     if reprs.first().is_none_or(|x| x.as_str() != "tuple_to_atom") {
//         return Err(RuleNotApplicable);
//     }
//
//     if reprs2.first().is_none_or(|x| x.as_str() != "tuple_to_atom") {
//         return Err(RuleNotApplicable);
//     }
//
//     // let decl = symbols.lookup(name).unwrap();
//     // let decl2 = symbols.lookup(name2).unwrap();
//
//     let domain = decl
//         .resolved_domain()
//         .ok_or(ApplicationError::DomainError)?;
//     let domain2 = decl2
//         .resolved_domain()
//         .ok_or(ApplicationError::DomainError)?;
//
//     let GroundDomain::Tuple(elems) = domain.as_ref() else {
//         return Err(RuleNotApplicable);
//     };
//
//     let GroundDomain::Tuple(elems2) = domain2.as_ref() else {
//         return Err(RuleNotApplicable);
//     };
//
//     if elems.len() != elems2.len() {
//         return Err(RuleNotApplicable);
//     }
//
//     let mut equality_constraints = vec![];
//     for i in 0..elems.len() {
//         let left_elem = Expression::SafeIndex(
//             Metadata::new(),
//             Moo::clone(left),
//             vec![Expression::Atomic(
//                 Metadata::new(),
//                 Atom::Literal(Literal::Int((i + 1) as i32)),
//             )],
//         );
//         let right_elem = Expression::SafeIndex(
//             Metadata::new(),
//             Moo::clone(right),
//             vec![Expression::Atomic(
//                 Metadata::new(),
//                 Atom::Literal(Literal::Int((i + 1) as i32)),
//             )],
//         );
//
//         equality_constraints.push(Expression::Eq(
//             Metadata::new(),
//             Moo::new(left_elem),
//             Moo::new(right_elem),
//         ));
//     }
//
//     let new_expr = Expression::And(
//         Metadata::new(),
//         Moo::new(into_matrix_expr!(equality_constraints)),
//     );
//
//     Ok(Reduction::pure(new_expr))
// }
//
// //tuple equality where the left is a variable and the right is a tuple literal
// #[register_rule(("Base", 2000))]
// fn tuple_to_constant(expr: &Expr, symbols: &SymbolTable) -> ApplicationResult {
//     let Expr::Eq(_, left, right) = expr else {
//         return Err(RuleNotApplicable);
//     };
//
//     let Expr::Atomic(_, Atom::Reference(decl)) = &**left else {
//         return Err(RuleNotApplicable);
//     };
//
//     let Name::WithRepresentation(name, reprs) = &decl.name() as &Name else {
//         return Err(RuleNotApplicable);
//     };
//
//     let Some(rhs_tuple_len) = crate::utils::constant_tuple_len(right.as_ref()) else {
//         return Err(RuleNotApplicable);
//     };
//
//     if reprs.first().is_none_or(|x| x.as_str() != "tuple_to_atom") {
//         return Err(RuleNotApplicable);
//     }
//
//     let decl = symbols.lookup(name).unwrap();
//
//     let domain = decl
//         .resolved_domain()
//         .ok_or(ApplicationError::DomainError)?;
//
//     let GroundDomain::Tuple(elems) = domain.as_ref() else {
//         return Err(RuleNotApplicable);
//     };
//
//     if elems.len() != rhs_tuple_len {
//         return Err(RuleNotApplicable);
//     }
//
//     let mut equality_constraints = vec![];
//     for i in 0..elems.len() {
//         let left_elem = Expression::SafeIndex(
//             Metadata::new(),
//             Moo::clone(left),
//             vec![Expression::Atomic(
//                 Metadata::new(),
//                 Atom::Literal(Literal::Int((i + 1) as i32)),
//             )],
//         );
//         let right_elem = Expression::SafeIndex(
//             Metadata::new(),
//             Moo::clone(right),
//             vec![Expression::Atomic(
//                 Metadata::new(),
//                 Atom::Literal(Literal::Int((i + 1) as i32)),
//             )],
//         );
//
//         equality_constraints.push(Expression::Eq(
//             Metadata::new(),
//             Moo::new(left_elem),
//             Moo::new(right_elem),
//         ));
//     }
//
//     let new_expr = Expression::And(
//         Metadata::new(),
//         Moo::new(into_matrix_expr!(equality_constraints)),
//     );
//
//     Ok(Reduction::pure(new_expr))
// }
//
// // convert equality to tuple inequality
// #[register_rule(("Base", 2000))]
// fn tuple_inequality(expr: &Expr, _: &SymbolTable) -> ApplicationResult {
//     let Expr::Neq(_, left, right) = expr else {
//         return Err(RuleNotApplicable);
//     };
//
//     let Expr::Atomic(_, Atom::Reference(decl)) = &**left else {
//         return Err(RuleNotApplicable);
//     };
//
//     let Name::WithRepresentation(_, reprs) = &decl.name() as &Name else {
//         return Err(RuleNotApplicable);
//     };
//
//     let Expr::Atomic(_, Atom::Reference(decl2)) = &**right else {
//         return Err(RuleNotApplicable);
//     };
//
//     let Name::WithRepresentation(_, reprs2) = &decl2.name() as &Name else {
//         return Err(RuleNotApplicable);
//     };
//
//     if reprs.first().is_none_or(|x| x.as_str() != "tuple_to_atom") {
//         return Err(RuleNotApplicable);
//     }
//
//     if reprs2.first().is_none_or(|x| x.as_str() != "tuple_to_atom") {
//         return Err(RuleNotApplicable);
//     }
//
//     let domain = decl
//         .resolved_domain()
//         .ok_or(ApplicationError::DomainError)?;
//
//     let domain2 = decl2
//         .resolved_domain()
//         .ok_or(ApplicationError::DomainError)?;
//
//     let GroundDomain::Tuple(elems) = domain.as_ref() else {
//         return Err(RuleNotApplicable);
//     };
//
//     let GroundDomain::Tuple(elems2) = domain2.as_ref() else {
//         return Err(RuleNotApplicable);
//     };
//
//     assert_eq!(
//         elems.len(),
//         elems2.len(),
//         "tuple inequality requires same length domains"
//     );
//
//     let mut equality_constraints = vec![];
//     for i in 0..elems.len() {
//         let left_elem = Expression::SafeIndex(
//             Metadata::new(),
//             Moo::clone(left),
//             vec![Expression::Atomic(
//                 Metadata::new(),
//                 Atom::Literal(Literal::Int((i + 1) as i32)),
//             )],
//         );
//         let right_elem = Expression::SafeIndex(
//             Metadata::new(),
//             Moo::clone(right),
//             vec![Expression::Atomic(
//                 Metadata::new(),
//                 Atom::Literal(Literal::Int((i + 1) as i32)),
//             )],
//         );
//
//         equality_constraints.push(Expression::Eq(
//             Metadata::new(),
//             Moo::new(left_elem),
//             Moo::new(right_elem),
//         ));
//     }
//
//     // Just copied from Conjure, would it be better to DeMorgan this?
//     let new_expr = Expression::Not(
//         Metadata::new(),
//         Moo::new(Expression::And(
//             Metadata::new(),
//             Moo::new(into_matrix_expr!(equality_constraints)),
//         )),
//     );
//
//     Ok(Reduction::pure(new_expr))
// }
