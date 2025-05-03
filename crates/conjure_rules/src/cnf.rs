/***********************************************************************************/
/*        This file contains rules for converting logic expressions to CNF         */
/***********************************************************************************/

use std::rc::Rc;

use conjure_core::solver::SolverFamily;
use conjure_core::{ast::Declaration, rule_engine::register_rule_set};

use conjure_core::ast::{Atom, Expression as Expr, Literal};
use conjure_core::metadata::Metadata;
use conjure_core::rule_engine::{
    register_rule, ApplicationError::RuleNotApplicable, ApplicationResult, Reduction,
};

use conjure_core::ast::AbstractLiteral::Matrix;
use conjure_core::ast::{Domain, SymbolTable};
use conjure_core::{into_matrix_expr, matrix_expr};

use crate::utils::is_literal;

register_rule_set!("CNF", ("Base"), (SolverFamily::SAT));

/// Converts an implication to cnf
///
/// ```text
/// x -> y ~~> !x \/ y
/// ```
#[register_rule(("CNF", 4100))]
fn remove_implication(expr: &Expr, _: &SymbolTable) -> ApplicationResult {
    let Expr::Imply(_, x, y) = expr else {
        return Err(RuleNotApplicable);
    };

    Ok(Reduction::pure(Expr::Or(
        Metadata::new(),
        Box::new(matrix_expr![
            Expr::Not(Metadata::new(), x.clone()),
            *y.clone()
        ]),
    )))
}

/// Converts an equivalence to cnf
///
/// ```text
/// x <-> y ~~> (x -> y) /\ (y -> x) ~~> (!x \/ y) /\ (!y \/ x)
///
/// This converts boolean expressions using equivalence to CNF.
/// ```
#[register_rule(("CNF", 4100))]
fn remove_equivalence(expr: &Expr, _: &SymbolTable) -> ApplicationResult {
    let Expr::Iff(_, x, y) = expr else {
        return Err(RuleNotApplicable);
    };

    Ok(Reduction::pure(Expr::And(
        Metadata::new(),
        Box::new(matrix_expr![
            Expr::Or(
                Metadata::new(),
                Box::new(matrix_expr![
                    Expr::Not(Metadata::new(), x.clone()),
                    *y.clone()
                ]),
            ),
            Expr::Or(
                Metadata::new(),
                Box::new(matrix_expr![
                    *x.clone(),
                    Expr::Not(Metadata::new(), y.clone())
                ]),
            )
        ]),
    )))
}

/// Converts an equals to cnf
///
/// ```text
/// x = y ~~> (x -> y) /\ (y -> x) ~~> (!x \/ y) /\ (!y \/ x)
///
/// This converts boolean expressions using equivalence to CNF.
/// ```
#[register_rule(("CNF", 4100))]
fn remove_equals(expr: &Expr, symbols: &SymbolTable) -> ApplicationResult {
    let Expr::Eq(_, x, y) = expr else {
        return Err(RuleNotApplicable);
    };

    let Some(Domain::BoolDomain) = x.domain_of(symbols) else {
        return Err(RuleNotApplicable);
    };

    let Some(Domain::BoolDomain) = y.domain_of(symbols) else {
        return Err(RuleNotApplicable);
    };

    Ok(Reduction::pure(Expr::And(
        Metadata::new(),
        Box::new(matrix_expr![
            Expr::Or(
                Metadata::new(),
                Box::new(matrix_expr![
                    Expr::Not(Metadata::new(), x.clone()),
                    *y.clone()
                ]),
            ),
            Expr::Or(
                Metadata::new(),
                Box::new(matrix_expr![
                    *x.clone(),
                    Expr::Not(Metadata::new(), y.clone())
                ]),
            )
        ]),
    )))
}

/// Converts an and/or expression to an aux variable, using the tseytin transformation
///
/// ```text
///  and(a, b, c, ...)
///  ~~>
///  __0
///
///  new variables:
///  find __0: bool
///
///  new constraints:
///  or(__0, not(a), not(b), not(c), ...)
///  or(not(__0), a)
///  or(not(__0), b)
///  or(not(__0), c)
///  ...
///
///  ---------------------------------------
///
///  or(a, b, c, ...)
///  ~~>
///  __0
///
///  new variables:
///  find __0: bool
///
///  new constraints:
///  or(not(__0), a, b, c, ...)
///  or(__0, not(a))
///  or(__0, not(b))
///  or(__0, not(c))
///  ...
/// ```
#[register_rule(("CNF", 8500))]
fn apply_tseytin_and_or(expr: &Expr, symbols: &SymbolTable) -> ApplicationResult {
    let exprs = match expr {
        Expr::And(_, exprs) | Expr::Or(_, exprs) => exprs,
        _ => return Err(RuleNotApplicable),
    };

    let Expr::AbstractLiteral(_, Matrix(exprs_list, _)) = exprs.as_ref() else {
        return Err(RuleNotApplicable);
    };

    for x in exprs_list {
        if !is_literal(x) {
            return Err(RuleNotApplicable);
        };
    }

    let new_expr;
    let new_tops;
    let new_symbols;

    match expr {
        Expr::And(_, _) => {
            (new_expr, new_symbols, new_tops) = tseytin_and(exprs_list, &symbols);
        }
        Expr::Or(_, _) => {
            (new_expr, new_symbols, new_tops) = tseytin_or(exprs_list, &symbols);
        }
        _ => return Err(RuleNotApplicable),
    };

    Ok(Reduction::new(new_expr, new_tops, new_symbols))
}

fn create_bool_aux(mut symbols: SymbolTable) -> (Expr, SymbolTable) {
    symbols = symbols.clone();
    let name = symbols.gensym();

    symbols.insert(Rc::new(Declaration::new_var(
        name.clone(),
        Domain::BoolDomain,
    )));

    (
        Expr::Atomic(Metadata::new(), Atom::Reference(name.clone())),
        symbols,
    )
}

/// Applies the Tseytin and transformation to series of variables, returns the new expression, symbol table and top level constraints
pub fn tseytin_and(exprs: &Vec<Expr>, symbols: &SymbolTable) -> (Expr, SymbolTable, Vec<Expr>) {
    let (new_expr, symbols) = create_bool_aux(symbols.clone());

    let mut new_tops: Vec<Expr> = vec![];
    let mut full_conj: Vec<Expr> = vec![new_expr.clone()];

    for x in exprs {
        new_tops.push(create_clause(vec![
            Expr::Not(Metadata::new(), Box::new(new_expr.clone())),
            x.clone(),
        ]));
        full_conj.push(Expr::Not(Metadata::new(), Box::new(x.clone())));
    }
    new_tops.push(create_clause(full_conj));

    (new_expr, symbols, new_tops)
}

/// Applies the Tseytin or transformation to series of variables, returns the new expression, symbol table and top level constraints
pub fn tseytin_or(exprs: &Vec<Expr>, symbols: &SymbolTable) -> (Expr, SymbolTable, Vec<Expr>) {
    let (new_expr, symbols) = create_bool_aux(symbols.clone());

    let mut new_tops: Vec<Expr> = vec![];
    let mut full_conj: Vec<Expr> = vec![Expr::Not(Metadata::new(), Box::new(new_expr.clone()))];

    for x in exprs {
        new_tops.push(create_clause(vec![
            Expr::Not(Metadata::new(), Box::new(x.clone())),
            new_expr.clone(),
        ]));
        full_conj.push(x.clone());
    }

    new_tops.push(create_clause(full_conj));

    (new_expr, symbols, new_tops)
}

/// Applies the Tseytin not transformation to a variable, returns the new expression, symbol table and top level constraints
pub fn tseytin_not(x: Expr, symbols: &SymbolTable) -> (Expr, SymbolTable, Vec<Expr>) {
    let (new_expr, symbols) = create_bool_aux(symbols.clone());

    let new_tops = vec![
        create_clause(vec![
            Expr::Not(Metadata::new(), Box::new(x.clone())),
            Expr::Not(Metadata::new(), Box::new(new_expr.clone())),
        ]),
        create_clause(vec![x.clone(), new_expr.clone()]),
    ];

    (new_expr, symbols, new_tops)
}

/// Applies the Tseytin iff transformation to two variables, returns the new expression, symbol table and top level constraints
pub fn tseytin_iff(x: Expr, y: Expr, symbols: &SymbolTable) -> (Expr, SymbolTable, Vec<Expr>) {
    let (new_expr, symbols) = create_bool_aux(symbols.clone());

    let new_tops = vec![
        create_clause(vec![
            Expr::Not(Metadata::new(), Box::new(x.clone())),
            Expr::Not(Metadata::new(), Box::new(y.clone())),
            new_expr.clone(),
        ]),
        create_clause(vec![x.clone(), y.clone(), new_expr.clone()]),
        create_clause(vec![
            x.clone(),
            Expr::Not(Metadata::new(), Box::new(y.clone())),
            Expr::Not(Metadata::new(), Box::new(new_expr.clone())),
        ]),
        create_clause(vec![
            Expr::Not(Metadata::new(), Box::new(x.clone())),
            y.clone(),
            Expr::Not(Metadata::new(), Box::new(new_expr.clone())),
        ]),
    ];

    (new_expr, symbols, new_tops)
}

/// Applies the Tseytin xor transformation to two variables, returns the new expression, symbol table and top level constraints
pub fn tseytin_xor(x: Expr, y: Expr, symbols: &SymbolTable) -> (Expr, SymbolTable, Vec<Expr>) {
    let (new_expr, symbols) = create_bool_aux(symbols.clone());

    let new_tops = vec![
        create_clause(vec![
            Expr::Not(Metadata::new(), Box::new(x.clone())),
            Expr::Not(Metadata::new(), Box::new(y.clone())),
            Expr::Not(Metadata::new(), Box::new(new_expr.clone())),
        ]),
        create_clause(vec![
            x.clone(),
            y.clone(),
            Expr::Not(Metadata::new(), Box::new(new_expr.clone())),
        ]),
        create_clause(vec![
            x.clone(),
            Expr::Not(Metadata::new(), Box::new(y.clone())),
            new_expr.clone(),
        ]),
        create_clause(vec![
            Expr::Not(Metadata::new(), Box::new(x.clone())),
            y.clone(),
            new_expr.clone(),
        ]),
    ];

    (new_expr, symbols, new_tops)
}

/// Applies the Tseytin imply transformation to two variables, returns the new expression, symbol table and top level constraints
pub fn tseytin_imply(x: Expr, y: Expr, symbols: &SymbolTable) -> (Expr, SymbolTable, Vec<Expr>) {
    let (new_expr, symbols) = create_bool_aux(symbols.clone());

    let new_tops = vec![
        create_clause(vec![
            Expr::Not(Metadata::new(), Box::new(new_expr.clone())),
            Expr::Not(Metadata::new(), Box::new(x.clone())),
            y.clone(),
        ]),
        create_clause(vec![new_expr.clone(), x.clone()]),
        create_clause(vec![
            new_expr.clone(),
            Expr::Not(Metadata::new(), Box::new(y.clone())),
        ]),
    ];

    (new_expr, symbols, new_tops)
}

fn create_clause(exprs: Vec<Expr>) -> Expr {
    let mut new_terms = vec![];
    for expr in exprs {
        if let Expr::Atomic(_, Atom::Literal(Literal::Bool(x))) = expr {
            // true ~~> entire or is true
            // false ~~> remove false from the or
            if x {
                return true.into();
            }
        } else if let Expr::Not(_, ref inner) = expr {
            if let Expr::Atomic(_, Atom::Literal(Literal::Bool(x))) = inner.as_ref() {
                // check for nested literal
                if !x {
                    return true.into();
                }
            } else {
                new_terms.push(expr);
            }
        } else {
            new_terms.push(expr);
        }
    }

    Expr::Clause(Metadata::new(), Box::new(into_matrix_expr![new_terms]))
}

/// Converts a not expression to an aux variable, using the tseytin transformation
///
/// ```text
///  not(a)
///  ~~>
///  __0
///
///  new variables:
///  find __0: bool
///
///  new constraints:
///  or(__0, a)
///  or(not(__0), not(a))
/// ```
// #[register_rule(("CNF", 8500))]
// fn apply_tseytin_not(expr: &Expr, symbols: &SymbolTable) -> ApplicationResult {
//     let Expr::Not(_, x) = expr else {
//         return Err(RuleNotApplicable);
//     };

//     let Expr::Atomic(_, _) = x.as_ref() else {
//         return Err(RuleNotApplicable);
//     };

//     let (new_expr, new_symbols, new_tops) = tseytin_not(*x.clone(), &symbols);

//     Ok(Reduction::new(new_expr, new_tops, new_symbols))
// }

/// Converts an iff expression to an aux variable, using the tseytin transformation
///
/// ```text
///  a <-> b
///  ~~>
///  __0
///
///  new variables:
///  find __0: bool
///
///  new constraints:
///  or(not(a), not(b), __0)
///  or(a, b, __0)
///  or(a, not(b), not(__0))
///  or(not(a), b, not(__0))
/// ```
#[register_rule(("CNF", 8500))]
fn apply_tseytin_iff(expr: &Expr, symbols: &SymbolTable) -> ApplicationResult {
    let Expr::Iff(_, x, y) = expr else {
        return Err(RuleNotApplicable);
    };

    if !is_literal(x.as_ref()) || !is_literal(y.as_ref()) {
        return Err(RuleNotApplicable);
    };

    let (new_expr, new_symbols, new_tops) = tseytin_iff(*x.clone(), *y.clone(), &symbols);

    Ok(Reduction::new(new_expr, new_tops, new_symbols))
}

/// Converts an implication expression to an aux variable, using the tseytin transformation
///
/// ```text
///  a -> b
///  ~~>
///  __0
///
///  new variables:
///  find __0: bool
///
///  new constraints:
///  or(not(__0), not(a), b)
///  or(__0, a)
///  or(__0, not(b))
/// ```
#[register_rule(("CNF", 8500))]
fn apply_tseytin_imply(expr: &Expr, symbols: &SymbolTable) -> ApplicationResult {
    let Expr::Imply(_, x, y) = expr else {
        return Err(RuleNotApplicable);
    };

    if !is_literal(x.as_ref()) || !is_literal(y.as_ref()) {
        return Err(RuleNotApplicable);
    };

    let (new_expr, new_symbols, new_tops) = tseytin_imply(*x.clone(), *y.clone(), &symbols);

    Ok(Reduction::new(new_expr, new_tops, new_symbols))
}

// #[register_rule(("CNF", 9100))]
// fn clause_partial_eval(expr: &Expr, _: &SymbolTable) -> ApplicationResult {
//     let Expr::Clause(_, e) = expr else {
//         return Err(RuleNotApplicable);
//     };

//     let Some(terms) = e.clone().unwrap_list() else {
//         return Err(RuleNotApplicable);
//     };

//     let mut has_changed = false;

//     // 2. boolean literals
//     let mut new_terms = vec![];
//     for expr in terms {
//         if let Expr::Atomic(_, Atom::Literal(Literal::Bool(x))) = expr {
//             has_changed = true;

//             // true ~~> entire or is true
//             // false ~~> remove false from the or
//             if x {
//                 return Ok(Reduction::pure(true.into()));
//             }
//         } else {
//             new_terms.push(expr);
//         }
//     }

//     // 3. empty clause ~~> false
//     if new_terms.is_empty() {
//         return Ok(Reduction::pure(false.into()));
//     }

//     if !has_changed {
//         return Err(RuleNotApplicable);
//     }

//     Ok(Reduction::pure(Expr::Clause(
//         Metadata::new(),
//         Box::new(into_matrix_expr![new_terms]),
//     )))
// }
