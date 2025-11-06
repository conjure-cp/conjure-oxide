use conjure_cp::ast::{
    AbstractLiteral, Atom, Domain, Expression, Literal, Metadata, Moo, Range, SymbolTable,
};
use conjure_cp::rule_engine::{
    ApplicationError, ApplicationResult, Reduction, register_rule, register_rule_set,
};
use conjure_cp::solver::SolverFamily;
use itertools::Itertools;

register_rule_set!("Smt", ("Base"), (SolverFamily::Smt));

#[register_rule(("Smt", 1000))]
fn expand_indomain(expr: &Expression, _: &SymbolTable) -> ApplicationResult {
    let Expression::InDomain(_, inner, domain) = expr else {
        return Err(ApplicationError::RuleNotApplicable);
    };

    let new_expr = match domain {
        // Bool values are always in the bool domain
        Domain::Bool => Ok(Expression::Atomic(
            Metadata::new(),
            Atom::Literal(Literal::Bool(true)),
        )),
        Domain::Empty(_) => Ok(Expression::Atomic(
            Metadata::new(),
            Atom::Literal(Literal::Bool(false)),
        )),
        Domain::Int(ranges) => {
            let elements: Vec<_> = ranges
                .iter()
                .map(|range| match range {
                    Range::Single(n) => {
                        let eq =
                            Expression::Atomic(Metadata::new(), Atom::Literal(Literal::Int(*n)));
                        Expression::Eq(Metadata::new(), inner.clone(), Moo::new(eq))
                    }
                    Range::Bounded(l, r) => {
                        let l_expr =
                            Expression::Atomic(Metadata::new(), Atom::Literal(Literal::Int(*l)));
                        let r_expr =
                            Expression::Atomic(Metadata::new(), Atom::Literal(Literal::Int(*r)));
                        let lit = AbstractLiteral::list(vec![
                            Expression::Geq(Metadata::new(), inner.clone(), Moo::new(l_expr)),
                            Expression::Leq(Metadata::new(), inner.clone(), Moo::new(r_expr)),
                        ]);
                        Expression::And(
                            Metadata::new(),
                            Moo::new(Expression::AbstractLiteral(Metadata::new(), lit)),
                        )
                    }
                    Range::UnboundedL(r) => {
                        let bound =
                            Expression::Atomic(Metadata::new(), Atom::Literal(Literal::Int(*r)));
                        Expression::Leq(Metadata::new(), inner.clone(), Moo::new(bound))
                    }
                    Range::UnboundedR(l) => {
                        let bound =
                            Expression::Atomic(Metadata::new(), Atom::Literal(Literal::Int(*l)));
                        Expression::Geq(Metadata::new(), inner.clone(), Moo::new(bound))
                    }
                })
                .collect();
            Ok(Expression::Or(
                Metadata::new(),
                Moo::new(Expression::AbstractLiteral(
                    Metadata::new(),
                    AbstractLiteral::list(elements),
                )),
            ))
        }
        _ => Err(ApplicationError::RuleNotApplicable),
    }?;
    Ok(Reduction::pure(new_expr))
}

/// E.g. for values a, b : matrix indexed by [bool, ...] of bool
/// a =/!= b ~> a[true, ...] = b[true, ...] /\ a[false, ...] = b[false, ...] /\ ...
#[register_rule(("Smt", 1000))]
fn expand_matrix_eq_neq(expr: &Expression, _: &SymbolTable) -> ApplicationResult {
    let (a, b) = match expr {
        Expression::Eq(_, a, b) | Expression::Neq(_, a, b) => (a, b),
        _ => return Err(ApplicationError::RuleNotApplicable),
    };

    let Some(Domain::Matrix(_, idx_domains)) = a.domain_of() else {
        return Err(ApplicationError::RuleNotApplicable);
    };

    let idx_lits: Result<Vec<Vec<Literal>>, _> = idx_domains.iter().map(Domain::values).collect();
    let idx_lits = idx_lits.map_err(|_| ApplicationError::DomainError)?;

    let pairs: Vec<_> = idx_lits
        .into_iter()
        .multi_cartesian_product()
        .map(|idx_lits| {
            let idx_vec: Vec<_> = idx_lits
                .into_iter()
                .map(|lit| Expression::Atomic(Metadata::new(), Atom::Literal(lit)))
                .collect();
            (
                Moo::new(Expression::SafeIndex(
                    Metadata::new(),
                    a.clone(),
                    idx_vec.clone(),
                )),
                Moo::new(Expression::SafeIndex(Metadata::new(), b.clone(), idx_vec)),
            )
        })
        .collect();

    let new_expr = match expr {
        Expression::Eq(_, _, _) => {
            let eqs: Vec<_> = pairs
                .into_iter()
                .map(|(a, b)| Expression::Eq(Metadata::new(), a, b))
                .collect();
            Expression::And(
                Metadata::new(),
                Moo::new(Expression::AbstractLiteral(
                    Metadata::new(),
                    AbstractLiteral::list(eqs),
                )),
            )
        }
        Expression::Neq(_, _, _) => {
            let neqs: Vec<_> = pairs
                .into_iter()
                .map(|(a, b)| Expression::Neq(Metadata::new(), a, b))
                .collect();
            Expression::Or(
                Metadata::new(),
                Moo::new(Expression::AbstractLiteral(
                    Metadata::new(),
                    AbstractLiteral::list(neqs),
                )),
            )
        }
        _ => unreachable!(),
    };

    Ok(Reduction::pure(new_expr))
}
