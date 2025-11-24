use conjure_cp::ast::{Atom, Expression as Expr, Metadata, Moo, SymbolTable};
use conjure_cp::rule_engine::{ApplicationError, ApplicationResult, Reduction, register_rule};

use ApplicationError::RuleNotApplicable;

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
