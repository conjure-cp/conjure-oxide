use conjure_cp::essence_expr;
use conjure_cp::rule_engine::register_rule_set;
use conjure_cp::solver::SolverFamily;

use conjure_cp::ast::Metadata;
use conjure_cp::ast::{Atom, CnfClause, Expression as Expr, Literal, Moo};
use conjure_cp::rule_engine::{
    ApplicationError::RuleNotApplicable, ApplicationResult, Reduction, register_rule,
};

use conjure_cp::ast::AbstractLiteral::Matrix;
use conjure_cp::ast::{Domain, SymbolTable};

use crate::utils::is_literal;

fn create_bool_aux(symbols: &mut SymbolTable) -> Expr {
    let name = symbols.gensym(&Domain::Bool);

    symbols.insert(name.clone());

    Expr::Atomic(Metadata::new(), Atom::Reference(name))
}

fn create_clause(exprs: Vec<Expr>) -> Option<CnfClause> {
    let mut new_terms = vec![];
    for expr in exprs {
        if let Expr::Atomic(_, Atom::Literal(Literal::Bool(x))) = expr {
            // true ~~> entire or is true
            // false ~~> remove false from the or
            if x {
                return None;
            }
        } else if let Expr::Not(_, ref inner) = expr {
            if let Expr::Atomic(_, Atom::Literal(Literal::Bool(x))) = inner.as_ref() {
                // check for nested literal
                if !x {
                    return None;
                }
            } else {
                new_terms.push(expr);
            }
        } else {
            new_terms.push(expr);
        }
    }

    Some(CnfClause::new(new_terms))
}

// TODO: Optimize all logic operators for constants
// TODO: If a clause simplifies to false, it should skip the solver and give no solutions

/// Applies the Tseytin and transformation to series of variables, returns the new expression, symbol table and clauses
pub fn tseytin_and(
    exprs: &Vec<Expr>,
    clauses: &mut Vec<CnfClause>,
    symbols: &mut SymbolTable,
) -> Expr {
    let new_expr = create_bool_aux(symbols);

    let mut full_conj: Vec<Expr> = vec![new_expr.clone()];

    for x in exprs {
        clauses.extend(create_clause(vec![
            Expr::Not(Metadata::new(), Moo::new(new_expr.clone())),
            x.clone(),
        ]));
        full_conj.push(Expr::Not(Metadata::new(), Moo::new(x.clone())));
    }
    clauses.extend(create_clause(full_conj));

    new_expr
}

/// Applies the Tseytin not transformation to a variable, returns the new expression, symbol table and clauses
pub fn tseytin_not(x: Expr, clauses: &mut Vec<CnfClause>, symbols: &mut SymbolTable) -> Expr {
    let new_expr = create_bool_aux(symbols);

    clauses.extend(create_clause(vec![
        Expr::Not(Metadata::new(), Moo::new(x.clone())),
        Expr::Not(Metadata::new(), Moo::new(new_expr.clone())),
    ]));
    clauses.extend(create_clause(vec![x, new_expr.clone()]));

    new_expr
}

/// Applies the Tseytin or transformation to series of variables, returns the new expression, symbol table and clauses
pub fn tseytin_or(
    exprs: &Vec<Expr>,
    clauses: &mut Vec<CnfClause>,
    symbols: &mut SymbolTable,
) -> Expr {
    let new_expr = create_bool_aux(symbols);

    let mut full_conj: Vec<Expr> = vec![Expr::Not(Metadata::new(), Moo::new(new_expr.clone()))];

    for x in exprs {
        clauses.extend(create_clause(vec![
            Expr::Not(Metadata::new(), Moo::new(x.clone())),
            new_expr.clone(),
        ]));
        full_conj.push(x.clone());
    }

    clauses.extend(create_clause(full_conj));

    new_expr
}

/// Applies the Tseytin iff transformation to two variables, returns the new expression, symbol table and clauses
pub fn tseytin_iff(
    x: Expr,
    y: Expr,
    clauses: &mut Vec<CnfClause>,
    symbols: &mut SymbolTable,
) -> Expr {
    let new_expr = create_bool_aux(symbols);

    clauses.extend(create_clause(vec![
        Expr::Not(Metadata::new(), Moo::new(x.clone())),
        Expr::Not(Metadata::new(), Moo::new(y.clone())),
        new_expr.clone(),
    ]));
    clauses.extend(create_clause(vec![x.clone(), y.clone(), new_expr.clone()]));
    clauses.extend(create_clause(vec![
        x.clone(),
        Expr::Not(Metadata::new(), Moo::new(y.clone())),
        Expr::Not(Metadata::new(), Moo::new(new_expr.clone())),
    ]));
    clauses.extend(create_clause(vec![
        Expr::Not(Metadata::new(), Moo::new(x)),
        y,
        Expr::Not(Metadata::new(), Moo::new(new_expr.clone())),
    ]));

    new_expr
}

/// Applies the Tseytin imply transformation to two variables, returns the new expression, symbol table and clauses
pub fn tseytin_imply(
    x: Expr,
    y: Expr,
    clauses: &mut Vec<CnfClause>,
    symbols: &mut SymbolTable,
) -> Expr {
    let new_expr = create_bool_aux(symbols);

    clauses.extend(create_clause(vec![
        Expr::Not(Metadata::new(), Moo::new(new_expr.clone())),
        Expr::Not(Metadata::new(), Moo::new(x.clone())),
        y.clone(),
    ]));
    clauses.extend(create_clause(vec![new_expr.clone(), x]));
    clauses.extend(create_clause(vec![
        new_expr.clone(),
        Expr::Not(Metadata::new(), Moo::new(y)),
    ]));

    new_expr
}

/// Applies the Tseytin multiplex transformation
/// cond ? b : a
///
/// cond = 1 => b
/// cond = 0 => a
#[allow(dead_code)]
pub fn tseytin_mux(
    cond: Expr,
    a: Expr,
    b: Expr,
    clauses: &mut Vec<CnfClause>,
    symbols: &mut SymbolTable,
) -> Expr {
    let new_expr = create_bool_aux(symbols);

    clauses.extend(create_clause(vec![
        Expr::Not(Metadata::new(), Moo::new(new_expr.clone())),
        cond.clone(),
        a.clone(),
    ]));
    clauses.extend(create_clause(vec![
        Expr::Not(Metadata::new(), Moo::new(new_expr.clone())),
        Expr::Not(Metadata::new(), Moo::new(cond.clone())),
        b.clone(),
    ]));
    clauses.extend(create_clause(vec![
        Expr::Not(Metadata::new(), Moo::new(new_expr.clone())),
        a.clone(),
        b.clone(),
    ]));

    clauses.extend(create_clause(vec![
        new_expr.clone(),
        cond.clone(),
        Expr::Not(Metadata::new(), Moo::new(a.clone())),
    ]));
    clauses.extend(create_clause(vec![
        new_expr.clone(),
        Expr::Not(Metadata::new(), Moo::new(cond)),
        Expr::Not(Metadata::new(), Moo::new(b.clone())),
    ]));
    clauses.extend(create_clause(vec![
        new_expr.clone(),
        Expr::Not(Metadata::new(), Moo::new(a)),
        Expr::Not(Metadata::new(), Moo::new(b)),
    ]));

    new_expr
}

/// Applies the Tseytin xor transformation to two variables, returns the new expression, symbol table and clauses
#[allow(dead_code)]
pub fn tseytin_xor(
    x: Expr,
    y: Expr,
    clauses: &mut Vec<CnfClause>,
    symbols: &mut SymbolTable,
) -> Expr {
    let new_expr = create_bool_aux(symbols);

    clauses.extend(create_clause(vec![
        Expr::Not(Metadata::new(), Moo::new(x.clone())),
        Expr::Not(Metadata::new(), Moo::new(y.clone())),
        Expr::Not(Metadata::new(), Moo::new(new_expr.clone())),
    ]));
    clauses.extend(create_clause(vec![
        x.clone(),
        y.clone(),
        Expr::Not(Metadata::new(), Moo::new(new_expr.clone())),
    ]));
    clauses.extend(create_clause(vec![
        x.clone(),
        Expr::Not(Metadata::new(), Moo::new(y.clone())),
        new_expr.clone(),
    ]));
    clauses.extend(create_clause(vec![
        Expr::Not(Metadata::new(), Moo::new(x)),
        y,
        new_expr.clone(),
    ]));

    new_expr
}

// BOOLEAN SAT ENCODING RULES:

register_rule_set!("SAT", ("Base"), (SolverFamily::Sat));

/// Converts a single boolean atom to a clause
///
/// ```text
///  a
///  ~~>
///  
///  new clauses:
///  clause(a)
/// ```
#[register_rule(("SAT", 8400))]
fn remove_single_atom(expr: &Expr, symbols: &SymbolTable) -> ApplicationResult {
    let Expr::Atomic(_, atom) = expr else {
        return Err(RuleNotApplicable);
    };

    let Atom::Reference(_) = atom else {
        return Err(RuleNotApplicable);
    };

    let new_clauses = vec![CnfClause::new(vec![expr.clone()])];
    let new_expr = essence_expr!(true);

    Ok(Reduction::cnf(new_expr, new_clauses, symbols.clone()))
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
///  new clauses:
///  clause(__0, not(a), not(b), not(c), ...)
///  clause(not(__0), a)
///  clause(not(__0), b)
///  clause(not(__0), c)
///  ...
///
///  ---------------------------------------
///
///  clause(a, b, c, ...)
///  ~~>
///  __0
///
///  new variables:
///  find __0: bool
///
///  new clauses:
///  clause(not(__0), a, b, c, ...)
///  clause(__0, not(a))
///  clause(__0, not(b))
///  clause(__0, not(c))
///  ...
/// ```
#[register_rule(("SAT", 8500))]
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
    let mut new_clauses = vec![];
    let mut new_symbols = symbols.clone();

    match expr {
        Expr::And(_, _) => {
            new_expr = tseytin_and(exprs_list, &mut new_clauses, &mut new_symbols);
        }
        Expr::Or(_, _) => {
            new_expr = tseytin_or(exprs_list, &mut new_clauses, &mut new_symbols);
        }
        _ => return Err(RuleNotApplicable),
    };

    Ok(Reduction::cnf(new_expr, new_clauses, new_symbols))
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
///  new clauses:
///  clause(__0, a)
///  clause(not(__0), not(a))
/// ```
#[register_rule(("SAT", 9005))]
fn apply_tseytin_not(expr: &Expr, symbols: &SymbolTable) -> ApplicationResult {
    let Expr::Not(_, x) = expr else {
        return Err(RuleNotApplicable);
    };

    let Expr::Atomic(_, _) = x.as_ref() else {
        return Err(RuleNotApplicable);
    };

    if !is_literal(x.as_ref()) {
        return Err(RuleNotApplicable);
    };

    let mut new_clauses = vec![];
    let mut new_symbols = symbols.clone();

    let new_expr = tseytin_not(x.as_ref().clone(), &mut new_clauses, &mut new_symbols);

    Ok(Reduction::cnf(new_expr, new_clauses, new_symbols))
}

/// Converts an iff expression to an aux variable, using the tseytin transformation
///
/// ```text
///  a <-> b
///  ~~>
///  __0
///
///  new clauses:
///  find __0: bool
///
///  new clauses:
///  clause(not(a), not(b), __0)
///  clause(a, b, __0)
///  clause(a, not(b), not(__0))
///  clause(not(a), b, not(__0))
/// ```
#[register_rule(("SAT", 8500))]
fn apply_tseytin_iff(expr: &Expr, symbols: &SymbolTable) -> ApplicationResult {
    let Expr::Iff(_, x, y) = expr else {
        return Err(RuleNotApplicable);
    };

    if !is_literal(x.as_ref()) || !is_literal(y.as_ref()) {
        return Err(RuleNotApplicable);
    };

    let mut new_clauses = vec![];
    let mut new_symbols = symbols.clone();

    let new_expr = tseytin_iff(
        x.as_ref().clone(),
        y.as_ref().clone(),
        &mut new_clauses,
        &mut new_symbols,
    );

    Ok(Reduction::cnf(new_expr, new_clauses, new_symbols))
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
///  new clauses:
///  clause(not(__0), not(a), b)
///  clause(__0, a)
///  clause(__0, not(b))
/// ```
#[register_rule(("SAT", 8500))]
fn apply_tseytin_imply(expr: &Expr, symbols: &SymbolTable) -> ApplicationResult {
    let Expr::Imply(_, x, y) = expr else {
        return Err(RuleNotApplicable);
    };

    if !is_literal(x.as_ref()) || !is_literal(y.as_ref()) {
        return Err(RuleNotApplicable);
    };

    let new_expr;
    let mut new_clauses = vec![];
    let mut new_symbols = symbols.clone();

    new_expr = tseytin_imply(
        x.as_ref().clone(),
        y.as_ref().clone(),
        &mut new_clauses,
        &mut new_symbols,
    );

    Ok(Reduction::cnf(new_expr, new_clauses, new_symbols))
}
