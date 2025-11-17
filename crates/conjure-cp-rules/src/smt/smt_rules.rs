use conjure_cp::ast::{Expression as Expr, *};
use conjure_cp::essence_expr;
use conjure_cp::rule_engine::ApplicationError;
use conjure_cp::rule_engine::{
    ApplicationError::{DomainError, RuleNotApplicable},
    ApplicationResult, Reduction, register_rule, register_rule_set,
};
use conjure_cp::solver::SolverFamily;
use uniplate::Uniplate;

register_rule_set!("Smt", ("Base"), (SolverFamily::Smt));

#[register_rule(("Smt", 1000))]
fn flatten_indomain(expr: &Expr, _: &SymbolTable) -> ApplicationResult {
    let Expr::InDomain(_, inner, domain) = expr else {
        return Err(RuleNotApplicable);
    };

    let new_expr = match domain {
        // Bool values are always in the bool domain
        Domain::Bool => Ok(Expr::Atomic(
            Metadata::new(),
            Atom::Literal(Literal::Bool(true)),
        )),
        Domain::Empty(_) => Ok(Expr::Atomic(
            Metadata::new(),
            Atom::Literal(Literal::Bool(false)),
        )),
        Domain::Int(ranges) => {
            let elements: Vec<_> = ranges
                .iter()
                .map(|range| match range {
                    Range::Single(n) => {
                        let eq = Expr::Atomic(Metadata::new(), Atom::Literal(Literal::Int(*n)));
                        Expr::Eq(Metadata::new(), inner.clone(), Moo::new(eq))
                    }
                    Range::Bounded(l, r) => {
                        let l_expr = Expr::Atomic(Metadata::new(), Atom::Literal(Literal::Int(*l)));
                        let r_expr = Expr::Atomic(Metadata::new(), Atom::Literal(Literal::Int(*r)));
                        let lit = AbstractLiteral::list(vec![
                            Expr::Geq(Metadata::new(), inner.clone(), Moo::new(l_expr)),
                            Expr::Leq(Metadata::new(), inner.clone(), Moo::new(r_expr)),
                        ]);
                        Expr::And(
                            Metadata::new(),
                            Moo::new(Expr::AbstractLiteral(Metadata::new(), lit)),
                        )
                    }
                    Range::UnboundedL(r) => {
                        let bound = Expr::Atomic(Metadata::new(), Atom::Literal(Literal::Int(*r)));
                        Expr::Leq(Metadata::new(), inner.clone(), Moo::new(bound))
                    }
                    Range::UnboundedR(l) => {
                        let bound = Expr::Atomic(Metadata::new(), Atom::Literal(Literal::Int(*l)));
                        Expr::Geq(Metadata::new(), inner.clone(), Moo::new(bound))
                    }
                })
                .collect();
            Ok(Expr::Or(
                Metadata::new(),
                Moo::new(Expr::AbstractLiteral(
                    Metadata::new(),
                    AbstractLiteral::list(elements),
                )),
            ))
        }
        _ => Err(RuleNotApplicable),
    }?;
    Ok(Reduction::pure(new_expr))
}

/// Matrix a = b iff every index in the union of their indices has the same value.
/// E.g. a: matrix indexed by [int(1..2)] of int(1..2), b: matrix indexed by [int(2..3)] of int(1..2)
/// a = b ~> a[1] = b[1] /\ a[2] = b[2] /\ a[3] = b[3]
#[register_rule(("Smt", 1000))]
fn flatten_matrix_eq_neq(expr: &Expr, _: &SymbolTable) -> ApplicationResult {
    let (a, b) = match expr {
        Expr::Eq(_, a, b) | Expr::Neq(_, a, b) => (a, b),
        _ => return Err(RuleNotApplicable),
    };

    let (Some(Domain::Matrix(_, a_idx_domains)), Some(Domain::Matrix(_, b_idx_domains))) =
        (a.domain_of(), b.domain_of())
    else {
        return Err(RuleNotApplicable);
    };

    let pairs = matrix::enumerate_index_union_indices(a_idx_domains, b_idx_domains)
        .map_err(|_| ApplicationError::DomainError)?
        .map(|idx_lits| {
            let idx_vec: Vec<_> = idx_lits
                .into_iter()
                .map(|lit| Atom::Literal(lit).into())
                .collect();
            (
                Expression::UnsafeIndex(Metadata::new(), a.clone(), idx_vec.clone()),
                Expression::UnsafeIndex(Metadata::new(), b.clone(), idx_vec),
            )
        });

    let new_expr = match expr {
        Expr::Eq(..) => {
            let eqs: Vec<_> = pairs.map(|(a, b)| essence_expr!(&a = &b)).collect();
            Expr::And(
                Metadata::new(),
                Moo::new(Expr::AbstractLiteral(
                    Metadata::new(),
                    AbstractLiteral::list(eqs),
                )),
            )
        }
        Expr::Neq(..) => {
            let neqs: Vec<_> = pairs.map(|(a, b)| essence_expr!(&a != &b)).collect();
            Expr::Or(
                Metadata::new(),
                Moo::new(Expr::AbstractLiteral(
                    Metadata::new(),
                    AbstractLiteral::list(neqs),
                )),
            )
        }
        _ => unreachable!(),
    };

    Ok(Reduction::pure(new_expr))
}

/// Turn a matrix slice into a 1-d matrix of the slice elements
/// E.g. m[1,..] ~> [m[1,1], m[1,2], m[1,3]]
#[register_rule(("Smt", 1000))]
fn flatten_matrix_slice(expr: &Expr, _: &SymbolTable) -> ApplicationResult {
    let Expr::SafeSlice(_, m, slice_idxs) = expr else {
        return Err(RuleNotApplicable);
    };
    let Some(Domain::Matrix(_, mat_idxs)) = m.domain_of() else {
        return Err(RuleNotApplicable);
    };

    if slice_idxs.len() != mat_idxs.len() {
        return Err(DomainError);
    }

    // Find where in the index vector the ".." is
    let (slice_dim, _) = slice_idxs
        .iter()
        .enumerate()
        .find(|(_, idx)| idx.is_none())
        .ok_or(RuleNotApplicable)?;
    let other_idxs = {
        let opt: Option<Vec<_>> = [&slice_idxs[..slice_dim], &slice_idxs[(slice_dim + 1)..]]
            .concat()
            .into_iter()
            .collect();
        opt.ok_or(DomainError)?
    };
    let elements: Vec<Expr> = mat_idxs[slice_dim]
        .values()
        .map_err(|_| DomainError)?
        .into_iter()
        .map(|lit| {
            let mut new_idx = other_idxs.clone();
            new_idx.insert(slice_dim, Expr::Atomic(Metadata::new(), Atom::Literal(lit)));
            Expr::SafeIndex(Metadata::new(), m.clone(), new_idx)
        })
        .collect();
    Ok(Reduction::pure(Expr::AbstractLiteral(
        Metadata::new(),
        AbstractLiteral::list(elements),
    )))
}

/// Expressions like allDiff and sum support 1-dimensional matrices as inputs, e.g. sum(m) where m is indexed by 1..3.
///
/// This rule is very similar to `matrix_ref_to_atom`, but turns the matrix reference into a slice rather its atoms.
/// Other rules like `flatten_matrix_slice` take care of actually turning the slice into the matrix elements.
#[register_rule(("Smt", 999))]
fn matrix_ref_to_slice(expr: &Expr, symbols: &SymbolTable) -> ApplicationResult {
    if let Expr::SafeSlice(_, _, _)
    | Expr::UnsafeSlice(_, _, _)
    | Expr::SafeIndex(_, _, _)
    | Expr::UnsafeIndex(_, _, _) = expr
    {
        return Err(RuleNotApplicable);
    };

    for (child, ctx) in expr.holes() {
        let Expr::Atomic(_, Atom::Reference(decl)) = &child else {
            continue;
        };

        let Some(Domain::Matrix(_, index_domains)) = decl.domain().map(|x| x.resolve(symbols))
        else {
            continue;
        };

        // Must be a 1d matrix
        if index_domains.len() > 1 {
            continue;
        }

        let new_child = Expr::SafeSlice(Metadata::new(), Moo::new(child.clone()), vec![None]);
        return Ok(Reduction::pure(ctx(new_child)));
    }

    Err(RuleNotApplicable)
}
