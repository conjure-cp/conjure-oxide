use conjure_cp_core::{
    ast::{Atom, Expression as Expr, Moo, SymbolTable},
    into_matrix_expr,
    metadata::Metadata,
    rule_engine::{
        ApplicationError, ApplicationResult, Reduction, register_rule, register_rule_set,
    },
};
use conjure_essence_macros::essence_expr;
use uniplate::Uniplate;

use ApplicationError::RuleNotApplicable;

register_rule_set!("Base", ());

/// This rule simplifies expressions where the operator is applied to an empty set of sub-expressions.
///
/// For example:
/// - `or([])` simplifies to `false` since no disjunction exists.
/// - `and([])` simplifies to `true` since no conjunction exists.
///
/// **Applicable examples:**
/// ```text
/// or([])  ~> false
/// X([]) ~> Nothing
/// ```
#[register_rule(("Base", 8800))]
fn remove_empty_expression(expr: &Expr, _: &SymbolTable) -> ApplicationResult {
    // excluded expressions
    if matches!(
        expr,
        Expr::Atomic(_, _)
            | Expr::Root(_, _)
            | Expr::Comprehension(_, _)
            | Expr::FlatIneq(_, _, _, _)
            | Expr::FlatMinusEq(_, _, _)
            | Expr::FlatSumGeq(_, _, _)
            | Expr::FlatSumLeq(_, _, _)
            | Expr::FlatProductEq(_, _, _, _)
            | Expr::FlatWatchedLiteral(_, _, _)
            | Expr::FlatWeightedSumGeq(_, _, _, _)
            | Expr::FlatWeightedSumLeq(_, _, _, _)
            | Expr::MinionDivEqUndefZero(_, _, _, _)
            | Expr::MinionModuloEqUndefZero(_, _, _, _)
            | Expr::MinionWInIntervalSet(_, _, _)
            | Expr::MinionWInSet(_, _, _)
            | Expr::MinionElementOne(_, _, _, _)
            | Expr::MinionPow(_, _, _, _)
            | Expr::MinionReify(_, _, _)
            | Expr::MinionReifyImply(_, _, _)
            | Expr::FlatAbsEq(_, _, _)
            | Expr::Min(_, _)
            | Expr::Max(_, _)
            | Expr::AllDiff(_, _)
            | Expr::FlatAllDiff(_, _)
            | Expr::AbstractLiteral(_, _)
    ) {
        return Err(ApplicationError::RuleNotApplicable);
    }

    if !expr.children().is_empty() {
        return Err(ApplicationError::RuleNotApplicable);
    }

    let new_expr = match expr {
        Expr::Or(_, _) => essence_expr!(false),
        Expr::And(_, _) => essence_expr!(true),
        _ => {
            return Err(ApplicationError::RuleNotApplicable);
        } // _ => And(Metadata::new(), Box::new(matrix_expr![])),
    };

    Ok(Reduction::pure(new_expr))
}

/**
 * Turn a Min into a new variable and post a top-level constraint to ensure the new variable is the minimum.
 * ```text
 * min([a, b]) ~> c ; c <= a & c <= b & (c = a | c = b)
 * ```
 */
#[register_rule(("Base", 6000))]
fn min_to_var(expr: &Expr, symbols: &SymbolTable) -> ApplicationResult {
    let Expr::Min(_, inside_min_expr) = expr else {
        return Err(RuleNotApplicable);
    };

    let Some(exprs) = inside_min_expr.as_ref().clone().unwrap_list() else {
        return Err(RuleNotApplicable);
    };

    let domain = expr.domain_of().ok_or(ApplicationError::DomainError)?;
    let mut symbols = symbols.clone();

    let atom_inner = Atom::new_ref(symbols.gensym(&domain));
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

    Ok(Reduction::new(
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
#[register_rule(("Base", 6000))]
fn max_to_var(expr: &Expr, symbols: &SymbolTable) -> ApplicationResult {
    let Expr::Max(_, inside_max_expr) = expr else {
        return Err(RuleNotApplicable);
    };

    let Some(exprs) = inside_max_expr.as_ref().clone().unwrap_list() else {
        return Err(RuleNotApplicable);
    };

    let domain = expr.domain_of().ok_or(ApplicationError::DomainError)?;
    let mut symbols: SymbolTable = symbols.clone();

    let atom_inner = Atom::new_ref(symbols.gensym(&domain));
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

    Ok(Reduction::new(essence_expr!(&atom_expr), new_top, symbols))
}
