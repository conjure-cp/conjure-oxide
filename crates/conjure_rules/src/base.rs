use std::rc::Rc;

use conjure_core::{
    ast::{Atom, Declaration, Expression as Expr, Literal as Lit, SymbolTable},
    into_matrix_expr, matrix_expr,
    metadata::Metadata,
    rule_engine::{
        register_rule, register_rule_set, ApplicationError, ApplicationResult, Reduction,
    },
};
use std::cell::RefCell;
use uniplate::Uniplate;

use ApplicationError::RuleNotApplicable;
use Atom::*;
use Expr::*;
use Lit::Bool;

register_rule_set!("Base", ());

/// This rule simplifies expressions where the operator is applied to an empty set of sub-expressions.
///
/// For example:
/// - `or([])` simplifies to `false` since no disjunction exists.
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
        Atomic(_, _)
            | Root(_, _)
            | Comprehension(_, _)
            | FlatIneq(_, _, _, _)
            | FlatMinusEq(_, _, _)
            | FlatSumGeq(_, _, _)
            | FlatSumLeq(_, _, _)
            | FlatProductEq(_, _, _, _)
            | FlatWatchedLiteral(_, _, _)
            | FlatWeightedSumGeq(_, _, _, _)
            | FlatWeightedSumLeq(_, _, _, _)
            | MinionDivEqUndefZero(_, _, _, _)
            | MinionModuloEqUndefZero(_, _, _, _)
            | MinionWInIntervalSet(_, _, _)
            | MinionElementOne(_, _, _, _)
            | MinionPow(_, _, _, _)
            | MinionReify(_, _, _)
            | MinionReifyImply(_, _, _)
            | FlatAbsEq(_, _, _)
            | Min(_, _)
            | Max(_, _)
            | AllDiff(_, _)
            | FlatAllDiff(_, _)
            | AbstractLiteral(_, _)
    ) {
        return Err(ApplicationError::RuleNotApplicable);
    }

    if !expr.children().is_empty() {
        return Err(ApplicationError::RuleNotApplicable);
    }

    let new_expr = match expr {
        Or(_, _) => Atomic(Metadata::new(), Literal(Bool(false))),
        _ => And(Metadata::new(), Box::new(matrix_expr![])),
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

    // let matrix expressions / comprehensions be unrolled first before applying this rule.
    let Some(exprs) = inside_min_expr.as_ref().clone().unwrap_list() else {
        return Err(RuleNotApplicable);
    };

    let mut symbols = symbols.clone();
    let new_name = symbols.gensym();

    let mut new_top = Vec::new(); // the new variable must be less than or equal to all the other variables
    let mut disjunction = Vec::new(); // the new variable must be equal to one of the variables
    for e in exprs {
        new_top.push(Leq(
            Metadata::new(),
            Box::new(Atomic(
                Metadata::new(),
                Reference(
                    new_name.clone(),
                    Rc::new(RefCell::new(Declaration::default())),
                ),
            )),
            Box::new(e.clone()),
        ));
        disjunction.push(Eq(
            Metadata::new(),
            Box::new(Atomic(
                Metadata::new(),
                Reference(
                    new_name.clone(),
                    Rc::new(RefCell::new(Declaration::default())),
                ),
            )),
            Box::new(e.clone()),
        ));
    }
    // TODO: deal with explicit index domains
    new_top.push(Or(
        Metadata::new(),
        Box::new(into_matrix_expr![disjunction]),
    ));

    let domain = expr
        .domain_of(&symbols)
        .ok_or(ApplicationError::DomainError)?;
    symbols.insert(Rc::new(Declaration::new_var(new_name.clone(), domain)));

    Ok(Reduction::new(
        Atomic(
            Metadata::new(),
            Reference(new_name, Rc::new(RefCell::new(Declaration::default()))),
        ),
        new_top,
        symbols,
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

    let mut symbols = symbols.clone();
    let new_name = symbols.gensym();

    let mut new_top = Vec::new(); // the new variable must be more than or equal to all the other variables
    let mut disjunction = Vec::new(); // the new variable must more than or equal to one of the variables
    for e in exprs {
        new_top.push(Geq(
            Metadata::new(),
            Box::new(Atomic(
                Metadata::new(),
                Reference(
                    new_name.clone(),
                    Rc::new(RefCell::new(Declaration::default())),
                ),
            )),
            Box::new(e.clone()),
        ));
        disjunction.push(Eq(
            Metadata::new(),
            Box::new(Atomic(
                Metadata::new(),
                Reference(
                    new_name.clone(),
                    Rc::new(RefCell::new(Declaration::default())),
                ),
            )),
            Box::new(e.clone()),
        ));
    }
    // FIXME: deal with explicitly given domains
    new_top.push(Or(
        Metadata::new(),
        Box::new(into_matrix_expr![disjunction]),
    ));

    let domain = expr
        .domain_of(&symbols)
        .ok_or(ApplicationError::DomainError)?;
    symbols.insert(Rc::new(Declaration::new_var(new_name.clone(), domain)));

    Ok(Reduction::new(
        Atomic(
            Metadata::new(),
            Reference(new_name, Rc::new(RefCell::new(Declaration::default()))),
        ),
        new_top,
        symbols,
    ))
}
