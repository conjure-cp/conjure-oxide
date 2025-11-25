use conjure_cp::ast::{Atom, Expression as Expr, Literal, Metadata, Moo, SymbolTable};
use conjure_cp::essence_expr;
use conjure_cp::rule_engine::{ApplicationError, ApplicationResult, Reduction, register_rule};

use ApplicationError::{DomainError, RuleNotApplicable};

use itertools::Itertools as _;

#[register_rule(("Base", 9000))]
fn normalise_lex_gt_geq(expr: &Expr, _: &SymbolTable) -> ApplicationResult {
    match expr {
        Expr::LexGt(metadata, a, b) => Ok(Reduction::pure(Expr::LexLt(
            metadata.clone_dirty(),
            b.clone(),
            a.clone(),
        ))),
        Expr::LexGeq(metadata, a, b) => Ok(Reduction::pure(Expr::LexLeq(
            metadata.clone_dirty(),
            b.clone(),
            a.clone(),
        ))),
        _ => Err(RuleNotApplicable),
    }
}

#[register_rule(("Minion", 2000))]
fn flatten_lex_lt_leq(expr: &Expr, _: &SymbolTable) -> ApplicationResult {
    let (a, b) = match expr {
        Expr::LexLt(_, a, b) | Expr::LexLeq(_, a, b) => (
            Moo::unwrap_or_clone(a.clone())
                .unwrap_list()
                .ok_or(RuleNotApplicable)?,
            Moo::unwrap_or_clone(b.clone())
                .unwrap_list()
                .ok_or(RuleNotApplicable)?,
        ),
        _ => return Err(RuleNotApplicable),
    };

    if a.len() != b.len() {
        return Err(ApplicationError::DomainError);
    }

    let atoms_a: Vec<Atom> = a
        .into_iter()
        .map(|e| e.try_into().map_err(|_| RuleNotApplicable))
        .collect::<Result<Vec<_>, ApplicationError>>()?;
    let atoms_b: Vec<Atom> = b
        .into_iter()
        .map(|e| e.try_into().map_err(|_| RuleNotApplicable))
        .collect::<Result<Vec<_>, ApplicationError>>()?;

    let new_expr = match expr {
        Expr::LexLt(..) => Expr::FlatLexLt(Metadata::new(), atoms_a, atoms_b),
        Expr::LexLeq(..) => Expr::FlatLexLeq(Metadata::new(), atoms_a, atoms_b),
        _ => unreachable!(),
    };

    Ok(Reduction::pure(new_expr))
}

/// Expand lexicographical lt/leq into a "recursive or" form
/// a <lex b ~> a[1] < b[1] \/ (a[1] = b[1] /\ (a[2] < b[2] \/ ( ... )))
///
/// If the matrices are different lengths, they can never be equal.
/// E.g. if |a| > |b| then a > b if they are equal for the length of b
///
/// If they are the same length, then the strictness of the comparison comes into effect.
#[register_rule(("Smt", 999))]
fn expand_lex_lt_leq(expr: &Expr, _: &SymbolTable) -> ApplicationResult {
    let (a, b) = match expr {
        Expr::LexLt(_, a, b) | Expr::LexLeq(_, a, b) => (a, b),
        _ => return Err(RuleNotApplicable),
    };

    let dom_a = a.domain_of().ok_or(RuleNotApplicable)?;
    let dom_b = b.domain_of().ok_or(RuleNotApplicable)?;

    let (Some((_, a_idx_domains)), Some((_, b_idx_domains))) =
        (dom_a.as_matrix_ground(), dom_b.as_matrix_ground())
    else {
        return Err(RuleNotApplicable);
    };

    if a_idx_domains.len() != 1 && b_idx_domains.len() != 1 {
        return Err(RuleNotApplicable);
    }

    let (a_idxs, b_idxs) = (
        a_idx_domains[0]
            .values()
            .map_err(|_| DomainError)?
            .collect_vec(),
        b_idx_domains[0]
            .values()
            .map_err(|_| DomainError)?
            .collect_vec(),
    );

    // If strict, then the base case where they are equal
    let or_eq = matches!(expr, Expr::LexLeq(..));
    let new_expr = lex_lt_to_recursive_or(a, b, &a_idxs, &b_idxs, or_eq);
    Ok(Reduction::pure(new_expr))
}

fn lex_lt_to_recursive_or(
    a: &Expr,
    b: &Expr,
    a_idxs: &[Literal],
    b_idxs: &[Literal],
    allow_eq: bool,
) -> Expr {
    match (a_idxs, b_idxs) {
        ([], []) => allow_eq.into(), // Base case: same length
        ([..], []) => false.into(),  // Base case: b is shorter
        ([], [..]) => true.into(),   // Base case: a is shorter

        ([a_idx, a_tail @ ..], [b_idx, b_tail @ ..]) => {
            let (a_at_idx, b_at_idx) = (
                Expr::SafeIndex(
                    Metadata::new(),
                    Moo::new(a.clone()),
                    vec![a_idx.clone().into()],
                ),
                Expr::SafeIndex(
                    Metadata::new(),
                    Moo::new(b.clone()),
                    vec![b_idx.clone().into()],
                ),
            );

            let tail = lex_lt_to_recursive_or(a, b, a_tail, b_tail, allow_eq);
            essence_expr!(r"&a_at_idx < &b_at_idx \/ (&a_at_idx = &b_at_idx /\ &tail)")
        }
    }
}
