use conjure_core::ast::Expression as Expr;
use conjure_core::ast::SymbolTable;
use conjure_core::rule_engine::{
    register_rule, ApplicationError::RuleNotApplicable, ApplicationResult, Reduction,
};
use itertools::izip;
use itertools::Itertools;

use crate::ast::Atom;
use crate::ast::Domain;
use crate::ast::Expression;
use crate::ast::Literal;
use crate::ast::Name;
use crate::into_matrix_expr;
use crate::matrix_expr;
use crate::metadata::Metadata;
use crate::rule_engine::ApplicationError;

//TODO: tuple I don't know priorities super well, fairly arbitary number from similar rules
#[register_rule(("Base", 2000))]
fn index_tuple_to_atom(expr: &Expr, symbols: &SymbolTable) -> ApplicationResult {
    // i assume the MkOpIndexing is the same as matrix indexing
    let Expr::SafeIndex(_, subject, indices) = expr else {
        return Err(RuleNotApplicable);
    };

    let Expr::Atomic(_, Atom::Reference(Name::WithRepresentation(name, reprs))) = &**subject else {
        return Err(RuleNotApplicable);
    };

    if reprs.first().is_none_or(|x| x.as_str() != "tuple_to_atom") {
        return Err(RuleNotApplicable);
    }

    let repr = symbols
        .get_representation(name, &["tuple_to_atom"])
        .unwrap()[0]
        .clone();

    let decl = symbols.lookup(name).unwrap();

    let Some(Domain::DomainTuple(_)) = decl.domain().cloned().map(|x| x.resolve(symbols)) else {
        return Err(RuleNotApplicable);
    };

    let mut indices_are_const = true;
    let mut indices_as_lits: Vec<Literal> = vec![];

    for index in indices {
        let Some(index) = index.clone().to_literal() else {
            indices_are_const = false;
            break;
        };
        indices_as_lits.push(index);
    }

    if indices_are_const {
        let indices_as_name = Name::RepresentedName(
            name.clone(),
            "tuple_to_atom".into(),
            indices_as_lits.iter().join("_"),
        );

        let subject = repr.expression_down(symbols)?[&indices_as_name].clone();

        Ok(Reduction::pure(subject))
    } else {
        todo!("can tuples even do this?") // TODO: tuple
    }
}

#[register_rule(("Bubble", 8000))]
fn tuple_index_to_bubble(expr: &Expr, symbols: &SymbolTable) -> ApplicationResult {
    let Expr::UnsafeIndex(_, subject, indices) = expr else {
        return Err(RuleNotApplicable);
    };
   

    let Expr::Atomic(_, Atom::Reference(Name::WithRepresentation(name, reprs))) = &**subject else {
        return Err(RuleNotApplicable);
    };

    if reprs.first().is_none_or(|x| x.as_str() != "tuple_to_atom") {
        return Err(RuleNotApplicable);
    }

    let domain = subject
        .domain_of(symbols)
        .ok_or(ApplicationError::DomainError)?;

   

    let Domain::DomainTuple(elems) = domain else {
        return Err(RuleNotApplicable);
    };

    assert_eq!(indices.len(), 1, "tuple indexing is always one dimensional");
    let index = indices[0].clone();

    let bubble_constraint = Box::new(Expression::And(
        Metadata::new(),
        Box::new(matrix_expr![
            Expression::Leq(
                Metadata::new(),
                Box::new(index.clone()),
                Box::new(Expression::Atomic(
                    Metadata::new(),
                    Atom::Literal(Literal::Int(elems.len() as i32))
                ))
            ),
            Expression::Geq(
                Metadata::new(),
                Box::new(index.clone()),
                Box::new(Expression::Atomic(
                    Metadata::new(),
                    Atom::Literal(Literal::Int(1))
                ))
            )
        ])),
    );

    let new_expr = Box::new(Expression::SafeIndex(
        Metadata::new(),
        subject.clone(),
        indices.clone(),
    ));
 
    Ok(Reduction::pure(Expression::Bubble(
        Metadata::new(),
        new_expr,
        bubble_constraint,
    )))

}
