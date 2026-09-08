use conjure_cp::essence_expr;
use conjure_cp::{
    ast::Metadata,
    ast::{Atom, Expression as Expr, Moo, SymbolTable},
    into_matrix_expr,
    rule_engine::{
        ApplicationError, ApplicationResult, RuleEffect, register_rule, register_rule_set,
    },
};

use ApplicationError::RuleNotApplicable;

register_rule_set!("Base", ());

/**
 * Turn a Min into a new variable and post a top-level constraint to ensure the new variable is the minimum.
 * ```text
 * min([a, b]) ~> c ; c <= a & c <= b & (c = a | c = b)
 * ```
 */
#[register_rule("Base", 6000, [Min])]
fn min_to_var(expr: &Expr, symbols: &SymbolTable) -> ApplicationResult {
    let Expr::Min(_, inside_min_expr) = expr else {
        return Err(RuleNotApplicable);
    };

    let Some(exprs) = inside_min_expr.as_ref().clone().unwrap_list() else {
        return Err(RuleNotApplicable);
    };

    let domain = expr.domain_of().ok_or(ApplicationError::DomainError)?;
    let mut symbols = symbols.clone();

    let atom_inner = Atom::new_ref(symbols.gen_find_auxiliary(&domain));
    let atom_expr = Expr::Atomic(Metadata::new(), atom_inner);

    let mut new_top = Vec::new();
    let mut disjunction = Vec::new();
    for e in exprs {
        // Use the Expr::Atomic version in constraints
        new_top.push(essence_expr!(&atom_expr <= &e));
        disjunction.push(essence_expr!(&atom_expr = &e));
    }
    // TODO: deal with explicit index domains
    new_top.push(Expr::Or(
        Metadata::new(),
        Moo::new(into_matrix_expr![disjunction]),
    ));

    Ok(RuleEffect::new(
        // Return the Expr::Atomic
        essence_expr!(&atom_expr),
        new_top,
        symbols.clone(),
    ))
}

/**
 * Turn a Max into a new variable and post a top level constraint to ensure the new variable is the maximum.
 * ```text
 * max([a, b]) ~> c ; c >= a & c >= b & (c = a | c = b)
 * ```
 */
#[register_rule("Base", 6000, [Max])]
fn max_to_var(expr: &Expr, symbols: &SymbolTable) -> ApplicationResult {
    let Expr::Max(_, inside_max_expr) = expr else {
        return Err(RuleNotApplicable);
    };

    let Some(exprs) = inside_max_expr.as_ref().clone().unwrap_list() else {
        return Err(RuleNotApplicable);
    };

    let domain = expr.domain_of().ok_or(ApplicationError::DomainError)?;
    let mut symbols: SymbolTable = symbols.clone();

    let atom_inner = Atom::new_ref(symbols.gen_find_auxiliary(&domain));
    let atom_expr = Expr::Atomic(Metadata::new(), atom_inner);

    let mut new_top = Vec::new(); // the new variable must be more than or equal to all the other variables
    let mut disjunction = Vec::new(); // the new variable must more than or equal to one of the variables
    for e in exprs {
        new_top.push(essence_expr!(&atom_expr >= &e));
        disjunction.push(essence_expr!(&atom_expr = &e));
    }
    // FIXME: deal with explicitly given domains
    new_top.push(Expr::Or(
        Metadata::new(),
        Moo::new(into_matrix_expr![disjunction]),
    ));

    Ok(RuleEffect::new(essence_expr!(&atom_expr), new_top, symbols))
}
