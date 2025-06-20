use conjure_core::ast::AbstractLiteral;
use conjure_core::ast::Expression as Expr;
use conjure_core::ast::SymbolTable;
use conjure_core::into_matrix_expr;
use conjure_core::matrix_expr;
use conjure_core::rule_engine::{
    register_rule, ApplicationError::RuleNotApplicable, ApplicationResult, Reduction,
};

use conjure_core::ast::Atom;
use conjure_core::ast::Domain;
use conjure_core::ast::Expression;
use conjure_core::ast::Literal;
use conjure_core::ast::Name;
use conjure_core::metadata::Metadata;
use conjure_core::rule_engine::ApplicationError;

//takes a safe index expression and converts it to an atom via the representation rules
#[register_rule(("Base", 2000))]
fn index_record_to_atom(expr: &Expr, symbols: &SymbolTable) -> ApplicationResult {
    let Expr::SafeIndex(_, subject, indices) = expr else {
        return Err(RuleNotApplicable);
    };

    let Expr::Atomic(_, Atom::Reference(Name::WithRepresentation(name, reprs))) = &**subject else {
        return Err(RuleNotApplicable);
    };

    if reprs.first().is_none_or(|x| x.as_str() != "record_to_atom") {
        return Err(RuleNotApplicable);
    }

    // tuples are always one dimensional
    if indices.len() != 1 {
        return Err(RuleNotApplicable);
    }

    let repr = symbols
        .get_representation(name, &["record_to_atom"])
        .unwrap()[0]
        .clone();

    let decl = symbols.lookup(name).unwrap();

    let Some(Domain::Record(_)) = decl.domain().cloned().map(|x| x.resolve(symbols)) else {
        return Err(RuleNotApplicable);
    };

    assert_eq!(
        indices.len(),
        1,
        "record indexing is always one dimensional"
    );

    let index = indices[0].clone();

    // during the conversion from unsafe index to safe index in bubbling
    // we convert the field name to a literal integer for direct access
    let Some(index) = index.clone().to_literal() else {
        return Err(RuleNotApplicable); // we don't support non-literal indices
    };

    let indices_as_name = Name::RepresentedName(Box::new((
        name.as_ref().clone(),
        "record_to_atom".into(),
        index.to_string(),
    )));

    let subject = repr.expression_down(symbols)?[&indices_as_name].clone();

    Ok(Reduction::pure(subject))
}

#[register_rule(("Bubble", 8000))]
fn record_index_to_bubble(expr: &Expr, symbols: &SymbolTable) -> ApplicationResult {
    let Expr::UnsafeIndex(_, subject, indices) = expr else {
        return Err(RuleNotApplicable);
    };

    let Expr::Atomic(_, Atom::Reference(Name::WithRepresentation(_, reprs))) = &**subject else {
        return Err(RuleNotApplicable);
    };

    if reprs.first().is_none_or(|x| x.as_str() != "record_to_atom") {
        return Err(RuleNotApplicable);
    }

    let domain = subject
        .domain_of(symbols)
        .ok_or(ApplicationError::DomainError)?;

    let Domain::Record(elems) = domain else {
        return Err(RuleNotApplicable);
    };

    assert_eq!(
        indices.len(),
        1,
        "record indexing is always one dimensional"
    );

    let index = indices[0].clone();

    let Expr::Atomic(_, Atom::Reference(name)) = index.clone() else {
        return Err(RuleNotApplicable);
    };

    // find what numerical index in elems matches the entry name
    let Some(idx) = elems.iter().position(|x| x.name == name) else {
        return Err(RuleNotApplicable);
    };

    // converting to an integer for direct access
    let idx = Expr::Atomic(Metadata::new(), Atom::Literal(Literal::Int(idx as i32 + 1)));

    let bubble_constraint = Box::new(Expression::And(
        Metadata::new(),
        Box::new(matrix_expr![
            Expression::Leq(
                Metadata::new(),
                Box::new(idx.clone()),
                Box::new(Expression::Atomic(
                    Metadata::new(),
                    Atom::Literal(Literal::Int(elems.len() as i32))
                ))
            ),
            Expression::Geq(
                Metadata::new(),
                Box::new(idx.clone()),
                Box::new(Expression::Atomic(
                    Metadata::new(),
                    Atom::Literal(Literal::Int(1))
                ))
            )
        ]),
    ));

    let new_expr = Box::new(Expression::SafeIndex(
        Metadata::new(),
        subject.clone(),
        Vec::from([idx]),
    ));

    Ok(Reduction::pure(Expression::Bubble(
        Metadata::new(),
        new_expr,
        bubble_constraint,
    )))
}

// dealing with equality over 2 record variables
#[register_rule(("Base", 2000))]
fn record_equality(expr: &Expr, symbols: &SymbolTable) -> ApplicationResult {
    let Expr::Eq(_, left, right) = expr else {
        return Err(RuleNotApplicable);
    };

    // check if both sides are record variables
    let Expr::Atomic(_, Atom::Reference(Name::WithRepresentation(name, reprs))) = &**left else {
        return Err(RuleNotApplicable);
    };

    let Expr::Atomic(_, Atom::Reference(Name::WithRepresentation(name2, reprs2))) = &**right else {
        return Err(RuleNotApplicable);
    };

    if reprs.first().is_none_or(|x| x.as_str() != "record_to_atom") {
        return Err(RuleNotApplicable);
    }

    if reprs2
        .first()
        .is_none_or(|x| x.as_str() != "record_to_atom")
    {
        return Err(RuleNotApplicable);
    }

    // grab both from the symbol table
    let decl = symbols.lookup(name).unwrap();
    let decl2 = symbols.lookup(name2).unwrap();

    // check both are are record variable domains
    let domain = decl
        .domain()
        .cloned()
        .map(|x| x.resolve(symbols))
        .ok_or(ApplicationError::DomainError)?;
    let domain2 = decl2
        .domain()
        .cloned()
        .map(|x| x.resolve(symbols))
        .ok_or(ApplicationError::DomainError)?;

    let Domain::Record(entries) = domain else {
        return Err(RuleNotApplicable);
    };

    let Domain::Record(entries2) = domain2 else {
        return Err(RuleNotApplicable);
    };

    // we only support equality over records of the same size
    if entries.len() != entries2.len() {
        return Err(RuleNotApplicable);
    }

    // assuming all record entry names must match for equality
    for i in 0..entries.len() {
        if entries[i].name != entries2[i].name {
            return Err(RuleNotApplicable);
        }
    }

    let mut equality_constraints = vec![];
    // unroll the equality into equality constraints for each field
    for i in 0..entries.len() {
        let left_elem = Expression::SafeIndex(
            Metadata::new(),
            Box::new(*left.clone()),
            vec![Expression::Atomic(
                Metadata::new(),
                Atom::Literal(Literal::Int((i + 1) as i32)),
            )],
        );
        let right_elem = Expression::SafeIndex(
            Metadata::new(),
            Box::new(*right.clone()),
            vec![Expression::Atomic(
                Metadata::new(),
                Atom::Literal(Literal::Int((i + 1) as i32)),
            )],
        );

        equality_constraints.push(Expression::Eq(
            Metadata::new(),
            Box::new(left_elem),
            Box::new(right_elem),
        ));
    }

    let new_expr = Expression::And(
        Metadata::new(),
        Box::new(into_matrix_expr!(equality_constraints)),
    );

    Ok(Reduction::pure(new_expr))
}

// dealing with equality where the left is a record variable, and the right is a constant record
#[register_rule(("Base", 2000))]
fn record_to_const(expr: &Expr, symbols: &SymbolTable) -> ApplicationResult {
    let Expr::Eq(_, left, right) = expr else {
        return Err(RuleNotApplicable);
    };

    let Expr::Atomic(_, Atom::Reference(Name::WithRepresentation(name, reprs))) = &**left else {
        return Err(RuleNotApplicable);
    };

    if reprs.first().is_none_or(|x| x.as_str() != "record_to_atom") {
        return Err(RuleNotApplicable);
    }

    let decl = symbols.lookup(name).unwrap();

    let domain = decl
        .domain()
        .cloned()
        .map(|x| x.resolve(symbols))
        .ok_or(ApplicationError::DomainError)?;

    let Domain::Record(entries) = domain else {
        return Err(RuleNotApplicable);
    };

    let Expr::AbstractLiteral(_, AbstractLiteral::Record(entries2)) = &**right else {
        return Err(RuleNotApplicable);
    };

    for i in 0..entries.len() {
        if entries[i].name != entries2[i].name {
            return Err(RuleNotApplicable);
        }
    }
    let mut equality_constraints = vec![];
    for i in 0..entries.len() {
        let left_elem = Expression::SafeIndex(
            Metadata::new(),
            Box::new(*left.clone()),
            vec![Expression::Atomic(
                Metadata::new(),
                Atom::Literal(Literal::Int((i + 1) as i32)),
            )],
        );
        let right_elem = Expression::SafeIndex(
            Metadata::new(),
            Box::new(*right.clone()),
            vec![Expression::Atomic(
                Metadata::new(),
                Atom::Literal(Literal::Int((i + 1) as i32)),
            )],
        );

        equality_constraints.push(Expression::Eq(
            Metadata::new(),
            Box::new(left_elem),
            Box::new(right_elem),
        ));
    }
    let new_expr = Expression::And(
        Metadata::new(),
        Box::new(into_matrix_expr!(equality_constraints)),
    );
    Ok(Reduction::pure(new_expr))
}
