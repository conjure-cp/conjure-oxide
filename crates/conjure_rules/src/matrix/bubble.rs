use conjure_core::ast::{Domain, Expression, SymbolTable};
use conjure_core::metadata::Metadata;
use conjure_core::rule_engine::{
    register_rule, ApplicationError, ApplicationError::RuleNotApplicable, ApplicationResult,
    Reduction,
};
use conjure_core::{bug, into_matrix_expr};
use itertools::{izip, Itertools as _};

/// Converts an unsafe index to a safe index using a bubble expression.
#[register_rule(("Bubble", 6000))]
fn index_to_bubble(expr: &Expression, symbols: &SymbolTable) -> ApplicationResult {
    let Expression::UnsafeIndex(_, subject, indices) = expr else {
        return Err(RuleNotApplicable);
    };

    let domain = subject
        .domain_of(symbols)
        .ok_or(ApplicationError::DomainError)?;

    // TODO: tuple, this is a hack right now just to avoid the rule being applied to tuples, but could we safely modify the rule to
    // handle tuples as well?
    if matches!(domain, Domain::Tuple(_)) || matches!(domain, Domain::Record(_)) {
        return Err(RuleNotApplicable);
    }

    let Domain::Matrix(_, index_domains) = domain else {
        bug!("subject of an index expression should have a matrix domain. subject: {:?}, with domain: {:?}", subject, domain);
    };

    assert_eq!(index_domains.len(),indices.len(),"in an index expression, there should be the same number of indices as the subject has index domains");

    let bubble_constraints = Box::new(into_matrix_expr![izip!(index_domains, indices)
        .map(|(domain, index)| {
            Expression::InDomain(Metadata::new(), Box::new(index.clone()), domain)
        })
        .collect_vec()]);

    let new_expr = Box::new(Expression::SafeIndex(
        Metadata::new(),
        subject.clone(),
        indices.clone(),
    ));

    Ok(Reduction::pure(Expression::Bubble(
        Metadata::new(),
        new_expr,
        Box::new(Expression::And(Metadata::new(), bubble_constraints)),
    )))
}

/// Converts an unsafe slice to a safe slice using a bubble expression.
#[register_rule(("Bubble", 6000))]
fn slice_to_bubble(expr: &Expression, symbols: &SymbolTable) -> ApplicationResult {
    let Expression::UnsafeSlice(_, subject, indices) = expr else {
        return Err(RuleNotApplicable);
    };

    let domain = subject
        .domain_of(symbols)
        .ok_or(ApplicationError::DomainError)?;

    let Domain::Matrix(_, index_domains) = domain else {
        bug!("subject of a slice expression should have a matrix domain. subject: {:?}, with domain: {:?}", subject, domain);
    };

    assert_eq!(index_domains.len(),indices.len(),"in a slice expression, there should be the same number of indices as the subject has index domains");

    // the wildcard dimension doesn't need a constraint.
    let bubble_constraints = Box::new(into_matrix_expr![izip!(index_domains, indices)
        .filter_map(|(domain, index)| {
            index
                .clone()
                .map(|index| Expression::InDomain(Metadata::new(), Box::new(index.clone()), domain))
        })
        .collect_vec()]);

    let new_expr = Box::new(Expression::SafeSlice(
        Metadata::new(),
        subject.clone(),
        indices.clone(),
    ));

    Ok(Reduction::pure(Expression::Bubble(
        Metadata::new(),
        new_expr,
        Box::new(Expression::And(Metadata::new(), bubble_constraints)),
    )))
}
