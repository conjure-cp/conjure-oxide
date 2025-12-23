use conjure_cp::ast::matrix::safe_index_optimised;
use conjure_cp::ast::{
    AbstractLiteral, Expression as Expr, GroundDomain, Metadata, Moo, SymbolTable,
};
use conjure_cp::essence_expr;
use conjure_cp::rule_engine::{
    ApplicationError::{DomainError, RuleNotApplicable},
    ApplicationResult, Reduction, register_rule, register_rule_set,
};
use conjure_cp::solver::SolverFamily;
use conjure_cp::solver::adaptors::smt::TheoryConfig;

// Only applicable when unwrap_alldiff is enabled in the SMT adaptor
register_rule_set!("SmtUnwrapAllDiff", ("Base"), |f: &SolverFamily| matches!(
    f,
    SolverFamily::Smt(TheoryConfig {
        unwrap_alldiff: true,
        ..
    })
));

#[register_rule(("SmtUnwrapAllDiff", 1000))]
fn unwrap_alldiff(expr: &Expr, _: &SymbolTable) -> ApplicationResult {
    let Expr::AllDiff(_, m) = expr else {
        return Err(RuleNotApplicable);
    };

    let dom = m.domain_of().ok_or(RuleNotApplicable)?;
    let Some(GroundDomain::Matrix(val_domain, index_domains)) =
        dom.resolve().map(Moo::unwrap_or_clone)
    else {
        return Err(RuleNotApplicable);
    };
    let [idx_domain] = index_domains.as_slice() else {
        return Err(DomainError);
    };

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

    Ok(Reduction::pure(essence_expr!("and(&clauses_list)")))
}
