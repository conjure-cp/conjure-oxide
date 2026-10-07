//! The two ways to give Z3 an `allDifferent`.
//!
//! Both rules sit in the `Smt` rule set at the same priority, so they are equally applicable to any
//! `allDifferent` and the heuristic picks between them: `-h i` asks, `-h x` enumerates both, and
//! `-h c`/`-h f` take the first. Whichever wins rewrites the expression, so the choice is recorded
//! in the model and is not asked again on the next visit.
//!
//! `distinct` is usually the better of the two, but not always -- Z3 sometimes does better with the
//! counting encoding, which is why this is a choice rather than a fixed lowering.

use conjure_cp::ast::matrix::safe_index_optimised;
use conjure_cp::ast::{
    AbstractLiteral, Expression as Expr, GroundDomain, Metadata, Moo, SymbolTable,
};
use conjure_cp::essence_expr;
use conjure_cp::rule_engine::{
    ApplicationError::{DomainError, RuleNotApplicable},
    ApplicationResult, RuleEffect, register_rule,
};

/// Hand `allDifferent` to Z3 as its native `distinct`.
#[register_rule("Smt", 4000, [AllDiff])]
fn alldiff_as_smt_distinct(expr: &Expr, _: &SymbolTable) -> ApplicationResult {
    let Expr::AllDiff(_, m) = expr else {
        return Err(RuleNotApplicable);
    };
    Ok(RuleEffect::pure(Expr::SmtDistinct(
        Metadata::new(),
        m.clone(),
    )))
}

/// Encode `allDifferent` as "no value occurs more than once" instead of using `distinct`.
#[register_rule("Smt", 4000, [AllDiff])]
fn unwrap_alldiff(expr: &Expr, _: &SymbolTable) -> ApplicationResult {
    let Expr::AllDiff(_, m) = expr else {
        return Err(RuleNotApplicable);
    };

    let dom = m.domain_of().ok_or(RuleNotApplicable)?;
    let Ok(GroundDomain::Matrix(val_domain, index_domains)) =
        dom.resolve().map(Moo::unwrap_or_clone)
    else {
        return Err(RuleNotApplicable);
    };
    let [idx_domain] = index_domains.as_slice() else {
        return Err(DomainError);
    };

    // Counting occurrences means enumerating the element domain and testing each entry against
    // every value in it. That only reads as `allDifferent` when the entries are scalars: for a
    // matrix of matrices the "values" are whole rows, and the encoding would compare a row against
    // whatever the chosen layout left each entry as. `distinct` handles those natively.
    if !matches!(
        val_domain.as_ref(),
        GroundDomain::Bool | GroundDomain::Int(_)
    ) {
        return Err(RuleNotApplicable);
    }

    let val_iter = val_domain.values().map_err(|_| DomainError)?;
    let clauses = val_iter
        .map(|lit| {
            let idx_iter = idx_domain.values().map_err(|_| DomainError)?;
            let occurences = idx_iter
                .map(|idx| {
                    let elem = safe_index_optimised(m.as_ref().clone(), idx).ok_or(DomainError)?;
                    Ok(essence_expr!("toInt(&elem = &lit)"))
                })
                .collect::<Result<Vec<_>, _>>()?;
            let occurences_list = Expr::AbstractLiteral(
                Metadata::new(),
                AbstractLiteral::matrix_implied_indices(occurences),
            );
            Ok(essence_expr!("sum(&occurences_list) <= 1"))
        })
        .collect::<Result<Vec<_>, _>>()?;
    let clauses_list = Expr::AbstractLiteral(
        Metadata::new(),
        AbstractLiteral::matrix_implied_indices(clauses),
    );

    Ok(RuleEffect::pure(essence_expr!("and(&clauses_list)")))
}
