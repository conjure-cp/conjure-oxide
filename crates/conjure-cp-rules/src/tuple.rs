use conjure_cp::ast::AbstractLiteral;
use conjure_cp::ast::Expression as Expr;
use conjure_cp::ast::Moo;
use conjure_cp::ast::SymbolTable;
use conjure_cp::into_matrix_expr;
use conjure_cp::matrix_expr;
use conjure_cp::rule_engine::{
    ApplicationError::RuleNotApplicable, ApplicationResult, Reduction, register_rule,
};

use conjure_cp::ast::Atom;
use conjure_cp::ast::Domain;
use conjure_cp::ast::Expression;
use conjure_cp::ast::Literal;
use conjure_cp::ast::Metadata;
use conjure_cp::ast::Name;
use conjure_cp::rule_engine::ApplicationError;

//TODO: largely copied from the matrix rules, This should be possible to simplify
#[register_rule(("Base", 2000))]
fn index_tuple_to_atom(expr: &Expr, symbols: &SymbolTable) -> ApplicationResult {
    // i assume the MkOpIndexing is the same as matrix indexing
    let Expr::SafeIndex(_, subject, indices) = expr else {
        return Err(RuleNotApplicable);
    };

    let Expr::Atomic(_, Atom::Reference(decl)) = &**subject else {
        return Err(RuleNotApplicable);
    };

    let Name::WithRepresentation(name, reprs) = &decl.name() as &Name else {
        return Err(RuleNotApplicable);
    };

    if reprs.first().is_none_or(|x| x.as_str() != "tuple_to_atom") {
        return Err(RuleNotApplicable);
    }

    // tuples are always one dimensional
    if indices.len() != 1 {
        return Err(RuleNotApplicable);
    }

    let repr = symbols
        .get_representation(name, &["tuple_to_atom"])
        .unwrap()[0]
        .clone();

    // let decl = symbols.lookup(name).unwrap();

    let Some(Domain::Tuple(_)) = decl.domain().map(|x| x.resolve(symbols)) else {
        return Err(RuleNotApplicable);
    };

    let mut indices_as_lit: Literal = Literal::Bool(false);

    for index in indices {
        let Some(index) = index.clone().into_literal() else {
            return Err(RuleNotApplicable); // we don't support non-literal indices
        };
        indices_as_lit = index;
    }

    let indices_as_name = Name::Represented(Box::new((
        name.as_ref().clone(),
        "tuple_to_atom".into(),
        indices_as_lit.into(),
    )));

    let subject = repr.expression_down(symbols)?[&indices_as_name].clone();

    Ok(Reduction::pure(subject))
}

#[register_rule(("Bubble", 8000))]
fn tuple_index_to_bubble(expr: &Expr, _: &SymbolTable) -> ApplicationResult {
    let Expr::UnsafeIndex(_, subject, indices) = expr else {
        return Err(RuleNotApplicable);
    };

    let Expr::Atomic(_, Atom::Reference(decl)) = &**subject else {
        return Err(RuleNotApplicable);
    };

    let Name::WithRepresentation(_, reprs) = &decl.name() as &Name else {
        return Err(RuleNotApplicable);
    };

    if reprs.first().is_none_or(|x| x.as_str() != "tuple_to_atom") {
        return Err(RuleNotApplicable);
    }

    let domain = subject.domain_of().ok_or(ApplicationError::DomainError)?;

    let Domain::Tuple(elems) = domain else {
        return Err(RuleNotApplicable);
    };

    assert_eq!(indices.len(), 1, "tuple indexing is always one dimensional");
    let index = indices[0].clone();

    let bubble_constraint = Moo::new(Expression::And(
        Metadata::new(),
        Moo::new(matrix_expr![
            Expression::Leq(
                Metadata::new(),
                Moo::new(index.clone()),
                Moo::new(Expression::Atomic(
                    Metadata::new(),
                    Atom::Literal(Literal::Int(elems.len() as i32))
                ))
            ),
            Expression::Geq(
                Metadata::new(),
                Moo::new(index),
                Moo::new(Expression::Atomic(
                    Metadata::new(),
                    Atom::Literal(Literal::Int(1))
                ))
            )
        ]),
    ));

    let new_expr = Moo::new(Expression::SafeIndex(
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

// convert equality to tuple equality
#[register_rule(("Base", 2000))]
fn tuple_equality(expr: &Expr, symbols: &SymbolTable) -> ApplicationResult {
    let Expr::Eq(_, left, right) = expr else {
        return Err(RuleNotApplicable);
    };

    let Expr::Atomic(_, Atom::Reference(decl)) = &**left else {
        return Err(RuleNotApplicable);
    };

    let Name::WithRepresentation(_, reprs) = &decl.name() as &Name else {
        return Err(RuleNotApplicable);
    };

    let Expr::Atomic(_, Atom::Reference(decl2)) = &**right else {
        return Err(RuleNotApplicable);
    };

    let Name::WithRepresentation(_, reprs2) = &decl2.name() as &Name else {
        return Err(RuleNotApplicable);
    };

    if reprs.first().is_none_or(|x| x.as_str() != "tuple_to_atom") {
        return Err(RuleNotApplicable);
    }

    if reprs2.first().is_none_or(|x| x.as_str() != "tuple_to_atom") {
        return Err(RuleNotApplicable);
    }

    // let decl = symbols.lookup(name).unwrap();
    // let decl2 = symbols.lookup(name2).unwrap();

    let domain = decl
        .domain()
        .map(|x| x.resolve(symbols))
        .ok_or(ApplicationError::DomainError)?;
    let domain2 = decl2
        .domain()
        .map(|x| x.resolve(symbols))
        .ok_or(ApplicationError::DomainError)?;

    let Domain::Tuple(elems) = domain else {
        return Err(RuleNotApplicable);
    };

    let Domain::Tuple(elems2) = domain2 else {
        return Err(RuleNotApplicable);
    };

    if elems.len() != elems2.len() {
        return Err(RuleNotApplicable);
    }

    let mut equality_constraints = vec![];
    for i in 0..elems.len() {
        let left_elem = Expression::SafeIndex(
            Metadata::new(),
            Moo::clone(left),
            vec![Expression::Atomic(
                Metadata::new(),
                Atom::Literal(Literal::Int((i + 1) as i32)),
            )],
        );
        let right_elem = Expression::SafeIndex(
            Metadata::new(),
            Moo::clone(right),
            vec![Expression::Atomic(
                Metadata::new(),
                Atom::Literal(Literal::Int((i + 1) as i32)),
            )],
        );

        equality_constraints.push(Expression::Eq(
            Metadata::new(),
            Moo::new(left_elem),
            Moo::new(right_elem),
        ));
    }

    let new_expr = Expression::And(
        Metadata::new(),
        Moo::new(into_matrix_expr!(equality_constraints)),
    );

    Ok(Reduction::pure(new_expr))
}

//tuple equality where the left is a variable and the right is a tuple literal
#[register_rule(("Base", 2000))]
fn tuple_to_constant(expr: &Expr, symbols: &SymbolTable) -> ApplicationResult {
    let Expr::Eq(_, left, right) = expr else {
        return Err(RuleNotApplicable);
    };

    let Expr::Atomic(_, Atom::Reference(decl)) = &**left else {
        return Err(RuleNotApplicable);
    };

    let Name::WithRepresentation(name, reprs) = &decl.name() as &Name else {
        return Err(RuleNotApplicable);
    };

    let Expr::AbstractLiteral(_, AbstractLiteral::Tuple(elems2)) = &**right else {
        return Err(RuleNotApplicable);
    };

    if reprs.first().is_none_or(|x| x.as_str() != "tuple_to_atom") {
        return Err(RuleNotApplicable);
    }

    let decl = symbols.lookup(name).unwrap();

    let domain = decl
        .domain()
        .map(|x| x.resolve(symbols))
        .ok_or(ApplicationError::DomainError)?;

    let Domain::Tuple(elems) = domain else {
        return Err(RuleNotApplicable);
    };

    if elems.len() != elems2.len() {
        return Err(RuleNotApplicable);
    }

    let mut equality_constraints = vec![];
    for i in 0..elems.len() {
        let left_elem = Expression::SafeIndex(
            Metadata::new(),
            Moo::clone(left),
            vec![Expression::Atomic(
                Metadata::new(),
                Atom::Literal(Literal::Int((i + 1) as i32)),
            )],
        );
        let right_elem = Expression::SafeIndex(
            Metadata::new(),
            Moo::clone(right),
            vec![Expression::Atomic(
                Metadata::new(),
                Atom::Literal(Literal::Int((i + 1) as i32)),
            )],
        );

        equality_constraints.push(Expression::Eq(
            Metadata::new(),
            Moo::new(left_elem),
            Moo::new(right_elem),
        ));
    }

    let new_expr = Expression::And(
        Metadata::new(),
        Moo::new(into_matrix_expr!(equality_constraints)),
    );

    Ok(Reduction::pure(new_expr))
}

// convert equality to tuple inequality
#[register_rule(("Base", 2000))]
fn tuple_inequality(expr: &Expr, symbols: &SymbolTable) -> ApplicationResult {
    let Expr::Neq(_, left, right) = expr else {
        return Err(RuleNotApplicable);
    };

    let Expr::Atomic(_, Atom::Reference(decl)) = &**left else {
        return Err(RuleNotApplicable);
    };

    let Name::WithRepresentation(_, reprs) = &decl.name() as &Name else {
        return Err(RuleNotApplicable);
    };

    let Expr::Atomic(_, Atom::Reference(decl2)) = &**right else {
        return Err(RuleNotApplicable);
    };

    let Name::WithRepresentation(_, reprs2) = &decl2.name() as &Name else {
        return Err(RuleNotApplicable);
    };

    if reprs.first().is_none_or(|x| x.as_str() != "tuple_to_atom") {
        return Err(RuleNotApplicable);
    }

    if reprs2.first().is_none_or(|x| x.as_str() != "tuple_to_atom") {
        return Err(RuleNotApplicable);
    }

    let domain = decl
        .domain()
        .map(|x| x.resolve(symbols))
        .ok_or(ApplicationError::DomainError)?;

    let domain2 = decl2
        .domain()
        .map(|x| x.resolve(symbols))
        .ok_or(ApplicationError::DomainError)?;

    let Domain::Tuple(elems) = domain else {
        return Err(RuleNotApplicable);
    };

    let Domain::Tuple(elems2) = domain2 else {
        return Err(RuleNotApplicable);
    };

    assert_eq!(
        elems.len(),
        elems2.len(),
        "tuple inequality requires same length domains"
    );

    let mut equality_constraints = vec![];
    for i in 0..elems.len() {
        let left_elem = Expression::SafeIndex(
            Metadata::new(),
            Moo::clone(left),
            vec![Expression::Atomic(
                Metadata::new(),
                Atom::Literal(Literal::Int((i + 1) as i32)),
            )],
        );
        let right_elem = Expression::SafeIndex(
            Metadata::new(),
            Moo::clone(right),
            vec![Expression::Atomic(
                Metadata::new(),
                Atom::Literal(Literal::Int((i + 1) as i32)),
            )],
        );

        equality_constraints.push(Expression::Eq(
            Metadata::new(),
            Moo::new(left_elem),
            Moo::new(right_elem),
        ));
    }

    // Just copied from Conjure, would it be better to DeMorgan this?
    let new_expr = Expression::Not(
        Metadata::new(),
        Moo::new(Expression::And(
            Metadata::new(),
            Moo::new(into_matrix_expr!(equality_constraints)),
        )),
    );

    Ok(Reduction::pure(new_expr))
}
