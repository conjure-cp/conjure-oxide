use conjure_cp::ast::{Atom, DomainPtr, GroundDomain, Metadata, Range, eval_constant};
use conjure_cp::ast::{Expression, Moo, SymbolTable};
use conjure_cp::rule_engine::{
    ApplicationError, ApplicationError::RuleNotApplicable, ApplicationResult, Reduction,
    register_rule,
};
use conjure_cp::{bug, into_matrix_expr};
use itertools::{Itertools as _, izip};

/// Whether an index expression is provably inside or outside a target domain.
#[derive(Debug, PartialEq, Eq)]
enum MembershipProof {
    AlwaysIn,
    AlwaysOut,
    Unknown,
}

/// Tries to prove whether `index` is always in, always out, or unknown for `domain`.
fn expression_membership_proof(
    domain: &GroundDomain,
    index: &Expression,
) -> Result<MembershipProof, ApplicationError> {
    let Some(index_domain) = index.domain_of().and_then(|domain| domain.resolve()) else {
        return Ok(MembershipProof::Unknown);
    };

    let Ok(intersection) = index_domain.as_ref().intersect(domain) else {
        return Ok(MembershipProof::Unknown);
    };

    if normalise_int_domain(&intersection) == normalise_int_domain(index_domain.as_ref()) {
        return Ok(MembershipProof::AlwaysIn);
    }

    if let Ok(values) = intersection.values_i32()
        && values.is_empty()
    {
        return Ok(MembershipProof::AlwaysOut);
    }

    Ok(MembershipProof::Unknown)
}

/// Normalises integer domains so equivalent range partitions compare equal.
fn normalise_int_domain(domain: &GroundDomain) -> GroundDomain {
    match domain {
        GroundDomain::Int(ranges) => GroundDomain::Int(Range::squeeze(
            &ranges
                .iter()
                .map(|range| Range::new(range.low().copied(), range.high().copied()))
                .collect_vec(),
        )),
        _ => domain.clone(),
    }
}

/// Builds the bubble condition needed to make an unsafe index operation safe.
fn index_bubble_condition(
    index_domains: &[Moo<GroundDomain>],
    indices: &[Expression],
) -> Result<Option<Expression>, ApplicationError> {
    let mut bubble_constraints = vec![];

    for (domain, index) in izip!(index_domains, indices) {
        match eval_constant(index) {
            Some(lit) => match domain
                .contains(&lit)
                .map_err(|_| ApplicationError::DomainError)?
            {
                true => {}
                false => {
                    return Ok(Some(Expression::Atomic(Metadata::new(), Atom::from(false))));
                }
            },
            None => match expression_membership_proof(domain.as_ref(), index)? {
                MembershipProof::AlwaysIn => {}
                MembershipProof::AlwaysOut => {
                    return Ok(Some(Expression::Atomic(Metadata::new(), Atom::from(false))));
                }
                MembershipProof::Unknown => bubble_constraints.push(Expression::InDomain(
                    Metadata::new(),
                    Moo::new(index.clone()),
                    DomainPtr::from(domain.clone()),
                )),
            },
        }
    }

    match bubble_constraints.len() {
        0 => Ok(None),
        1 => Ok(Some(
            bubble_constraints.pop().expect("length checked above"),
        )),
        _ => Ok(Some(Expression::And(
            Metadata::new(),
            Moo::new(into_matrix_expr![bubble_constraints]),
        ))),
    }
}

/// Converts an unsafe index to a safe index using a bubble expression.
#[register_rule("Bubble", 6000, [UnsafeIndex])]
fn index_to_bubble(expr: &Expression, _: &SymbolTable) -> ApplicationResult {
    let Expression::UnsafeIndex(_, subject, indices) = expr else {
        return Err(RuleNotApplicable);
    };

    let domain = subject
        .domain_of()
        .ok_or(ApplicationError::DomainError)?
        .resolve()
        .ok_or(RuleNotApplicable)?;

    // TODO: tuple, this is a hack right now just to avoid the rule being applied to tuples, but could we safely modify the rule to
    // handle tuples as well?
    if matches!(domain.as_ref(), GroundDomain::Tuple(_))
        || matches!(domain.as_ref(), GroundDomain::Record(_))
    {
        return Err(RuleNotApplicable);
    }

    let GroundDomain::Matrix(_, index_domains) = domain.as_ref() else {
        bug!(
            "subject of an index expression should have a matrix domain. subject: {:?}, with domain: {:?}",
            subject,
            domain.as_ref()
        );
    };

    assert_eq!(
        index_domains.len(),
        indices.len(),
        "in an index expression, there should be the same number of indices as the subject has index domains"
    );

    let new_expr = Moo::new(Expression::SafeIndex(
        Metadata::new(),
        subject.clone(),
        indices.clone(),
    ));

    match index_bubble_condition(index_domains, indices)? {
        None => Ok(Reduction::pure(Moo::unwrap_or_clone(new_expr))),
        Some(condition) => Ok(Reduction::pure(Expression::Bubble(
            Metadata::new(),
            new_expr,
            Moo::new(condition),
        ))),
    }
}

/// Converts an unsafe slice to a safe slice using a bubble expression.
#[register_rule("Bubble", 6000, [UnsafeSlice])]
fn slice_to_bubble(expr: &Expression, _: &SymbolTable) -> ApplicationResult {
    let Expression::UnsafeSlice(_, subject, indices) = expr else {
        return Err(RuleNotApplicable);
    };

    let domain = subject
        .domain_of()
        .ok_or(ApplicationError::DomainError)?
        .resolve()
        .ok_or(RuleNotApplicable)?;

    let GroundDomain::Matrix(_, index_domains) = domain.as_ref() else {
        bug!(
            "subject of a slice expression should have a matrix domain. subject: {:?}, with domain: {:?}",
            subject,
            domain
        );
    };

    assert_eq!(
        index_domains.len(),
        indices.len(),
        "in a slice expression, there should be the same number of indices as the subject has index domains"
    );

    let constrained_index_domains = izip!(index_domains, indices)
        .filter_map(|(domain, index)| index.clone().map(|index| (domain.clone(), index)))
        .collect_vec();
    let (filtered_index_domains, filtered_indices): (Vec<_>, Vec<_>) =
        constrained_index_domains.into_iter().unzip();

    let new_expr = Moo::new(Expression::SafeSlice(
        Metadata::new(),
        subject.clone(),
        indices.clone(),
    ));

    match index_bubble_condition(&filtered_index_domains, &filtered_indices)? {
        None => Ok(Reduction::pure(Moo::unwrap_or_clone(new_expr))),
        Some(condition) => Ok(Reduction::pure(Expression::Bubble(
            Metadata::new(),
            new_expr,
            Moo::new(condition),
        ))),
    }
}
