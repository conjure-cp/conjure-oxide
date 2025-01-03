use conjure_core::ast::{Atom, DecisionVariable, Expression as Expr, Literal as Lit, SymbolTable};
use conjure_core::metadata::Metadata;
use conjure_core::rule_engine::{
    register_rule, register_rule_set, ApplicationError, ApplicationResult, Reduction,
};
use conjure_core::Model;
use uniplate::Uniplate;

use Atom::*;
use Expr::*;
use Lit::*;

register_rule_set!("Base", 100, ());

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
fn remove_empty_expression(expr: &Expr, _: &Model) -> ApplicationResult {
    // excluded expressions
    if matches!(
        expr,
        Atomic(_, _)
            | FlatIneq(_, _, _, _)
            | FlatMinusEq(_, _, _)
            | FlatSumGeq(_, _, _)
            | FlatSumLeq(_, _, _)
            | FlatWatchedLiteral(_, _, _)
            | MinionDivEqUndefZero(_, _, _, _)
            | MinionModuloEqUndefZero(_, _, _, _)
            | MinionReify(_, _, _)
    ) {
        return Err(ApplicationError::RuleNotApplicable);
    }

    if !expr.children().is_empty() {
        return Err(ApplicationError::RuleNotApplicable);
    }

    let new_expr = match expr {
        Or(_, _) => Atomic(Metadata::new(), Literal(Bool(false))),
        _ => And(Metadata::new(), vec![]), // TODO: (yb33) Change it to a simple vector after we refactor our model,
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
fn min_to_var(expr: &Expr, mdl: &Model) -> ApplicationResult {
    match expr {
        Min(_, exprs) => {
            let new_name = mdl.gensym();

            let mut new_top = Vec::new(); // the new variable must be less than or equal to all the other variables
            let mut disjunction = Vec::new(); // the new variable must be equal to one of the variables
            for e in exprs {
                new_top.push(Leq(
                    Metadata::new(),
                    Box::new(Atomic(Metadata::new(), Reference(new_name.clone()))),
                    Box::new(e.clone()),
                ));
                disjunction.push(Eq(
                    Metadata::new(),
                    Box::new(Atomic(Metadata::new(), Reference(new_name.clone()))),
                    Box::new(e.clone()),
                ));
            }
            new_top.push(Or(Metadata::new(), disjunction));

            let mut new_vars = SymbolTable::new();
            let domain = expr
                .domain_of(&mdl.variables)
                .ok_or(ApplicationError::DomainError)?;
            new_vars.insert(new_name.clone(), DecisionVariable::new(domain));

            Ok(Reduction::new(
                Atomic(Metadata::new(), Reference(new_name)),
                new_top,
                new_vars,
            ))
        }
        _ => Err(ApplicationError::RuleNotApplicable),
    }
}

/**
 * Turn a Max into a new variable and post a top level constraint to ensure the new variable is the maximum.
 * ```text
 * max([a, b]) ~> c ; c >= a & c >= b & (c = a | c = b)
 * ```
 */
#[register_rule(("Base", 6000))]
fn max_to_var(expr: &Expr, mdl: &Model) -> ApplicationResult {
    match expr {
        Max(_, exprs) => {
            let new_name = mdl.gensym();

            let mut new_top = Vec::new(); // the new variable must be more than or equal to all the other variables
            let mut disjunction = Vec::new(); // the new variable must more than or equal to one of the variables
            for e in exprs {
                new_top.push(Geq(
                    Metadata::new(),
                    Box::new(Atomic(Metadata::new(), Reference(new_name.clone()))),
                    Box::new(e.clone()),
                ));
                disjunction.push(Eq(
                    Metadata::new(),
                    Box::new(Atomic(Metadata::new(), Reference(new_name.clone()))),
                    Box::new(e.clone()),
                ));
            }
            new_top.push(Or(Metadata::new(), disjunction));

            let mut new_vars = SymbolTable::new();
            let domain = expr
                .domain_of(&mdl.variables)
                .ok_or(ApplicationError::DomainError)?;
            new_vars.insert(new_name.clone(), DecisionVariable::new(domain));

            Ok(Reduction::new(
                Atomic(Metadata::new(), Reference(new_name)),
                new_top,
                new_vars,
            ))
        }
        _ => Err(ApplicationError::RuleNotApplicable),
    }
}
