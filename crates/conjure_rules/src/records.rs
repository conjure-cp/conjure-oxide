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


#[register_rule(("Base", 2000))]
fn index_record_to_atom(expr: &Expr, symbols: &SymbolTable) -> ApplicationResult {

   
    // i assume the MkOpIndexing is the same as matrix indexing
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

    let Some(Domain::DomainRecord(entries)) = decl.domain().cloned().map(|x| x.resolve(symbols)) else {
        return Err(RuleNotApplicable);
    };

    let mut indices_as_lit: Literal = Literal::Bool(false);

    assert_eq!(indices.len(), 1, "record indexing is always one dimensional");

    let index = indices[0].clone();

    let Expr::Atomic(_, Atom::Reference(nam) )= index.clone() else {
        
        return Err(RuleNotApplicable);
    };
    println!("index name: {:?}", nam);

    // find what numerical index in elems matches the entry name
    let Some(idx) = entries.iter().position(|x| x.name == nam) else {
        return Err(RuleNotApplicable);
    };
    println!("index: {:?}", idx);

    indices_as_lit = Literal::Int(idx as i32 +1);


    let indices_as_name = Name::RepresentedName(
        name.clone(),
        "record_to_atom".into(),
        indices_as_lit.to_string(),
    );

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

    let Domain::DomainRecord(elems) = domain else {
        return Err(RuleNotApplicable);
    };

    assert_eq!(indices.len(), 1, "tuple indexing is always one dimensional");
    let index = indices[0].clone();
    println!("index: {:?}", index);

    // how do i convert a reference to a name into a raw name to then check against the name in the records?

    let Expr::Atomic(_, Atom::Reference(nam) )= index.clone() else {
        
        return Err(RuleNotApplicable);
    };
    println!("index name: {:?}", nam);

    // find what numerical index in elems matches the entry name
    let Some(idx) = elems.iter().position(|x| x.name == nam) else {
        return Err(RuleNotApplicable);
    };
    println!("index: {:?}", idx);


    // idx as a int literal
    let idx = Expr::Atomic(Metadata::new(),  Atom::Literal(Literal::Int(idx as i32 +1)));

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
        indices.clone(),
    ));

    Ok(Reduction::pure(Expression::Bubble(
        Metadata::new(),
        new_expr,
        bubble_constraint,
    )))
}