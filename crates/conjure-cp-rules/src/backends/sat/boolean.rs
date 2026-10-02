use conjure_cp::essence_expr;

use conjure_cp::ast::Metadata;
use conjure_cp::ast::{Atom, Expression as Expr, Moo, SatEncodingDecision};
use conjure_cp::rule_engine::{
    ApplicationError::RuleNotApplicable, ApplicationResult, RuleEffect, register_rule,
};

use conjure_cp::ast::AbstractLiteral::Matrix;
use conjure_cp::ast::{Domain, SymbolTable};

use crate::shared::utils::is_literal;

fn create_bool_aux(symbols: &mut SymbolTable) -> Expr {
    let name = symbols.gen_find_auxiliary(&Domain::bool());

    symbols.insert(name.clone());

    Expr::Atomic(
        Metadata::new(),
        Atom::Reference(conjure_cp::ast::Reference::new(name)),
    )
}

/// Record an AND gate without generating clauses.
pub fn tseytin_and(
    exprs: &Vec<Expr>,
    decisions: &mut Vec<SatEncodingDecision>,
    symbols: &mut SymbolTable,
) -> Expr {
    gate(
        Expr::And(
            Metadata::new(),
            Moo::new(conjure_cp::into_matrix_expr!(exprs.clone())),
        ),
        decisions,
        symbols,
    )
}

/// Record a NOT gate without generating clauses.
pub fn tseytin_not(
    x: Expr,
    decisions: &mut Vec<SatEncodingDecision>,
    symbols: &mut SymbolTable,
) -> Expr {
    gate(Expr::Not(Metadata::new(), Moo::new(x)), decisions, symbols)
}

/// Record an OR gate without generating clauses.
pub fn tseytin_or(
    exprs: &Vec<Expr>,
    decisions: &mut Vec<SatEncodingDecision>,
    symbols: &mut SymbolTable,
) -> Expr {
    gate(
        Expr::Or(
            Metadata::new(),
            Moo::new(conjure_cp::into_matrix_expr!(exprs.clone())),
        ),
        decisions,
        symbols,
    )
}

/// Record a Boolean equivalence gate.
pub fn tseytin_iff(
    x: Expr,
    y: Expr,
    decisions: &mut Vec<SatEncodingDecision>,
    symbols: &mut SymbolTable,
) -> Expr {
    gate(
        Expr::Iff(Metadata::new(), Moo::new(x), Moo::new(y)),
        decisions,
        symbols,
    )
}

/// Record a Boolean implication gate.
pub fn tseytin_imply(
    x: Expr,
    y: Expr,
    decisions: &mut Vec<SatEncodingDecision>,
    symbols: &mut SymbolTable,
) -> Expr {
    gate(
        Expr::Imply(Metadata::new(), Moo::new(x), Moo::new(y)),
        decisions,
        symbols,
    )
}

/// Record a conditional Boolean choice.
#[allow(dead_code)]
pub fn tseytin_mux(
    cond: Expr,
    a: Expr,
    b: Expr,
    decisions: &mut Vec<SatEncodingDecision>,
    symbols: &mut SymbolTable,
) -> Expr {
    let when_true = Expr::And(
        Metadata::new(),
        Moo::new(conjure_cp::into_matrix_expr!(vec![cond.clone(), b])),
    );
    let when_false = Expr::And(
        Metadata::new(),
        Moo::new(conjure_cp::into_matrix_expr!(vec![
            Expr::Not(Metadata::new(), Moo::new(cond)),
            a
        ])),
    );
    gate(
        Expr::Or(
            Metadata::new(),
            Moo::new(conjure_cp::into_matrix_expr!(vec![when_true, when_false])),
        ),
        decisions,
        symbols,
    )
}

/// Record a Boolean exclusive-or gate.
#[allow(dead_code)]
pub fn tseytin_xor(
    x: Expr,
    y: Expr,
    decisions: &mut Vec<SatEncodingDecision>,
    symbols: &mut SymbolTable,
) -> Expr {
    gate(
        Expr::Not(
            Metadata::new(),
            Moo::new(Expr::Iff(Metadata::new(), Moo::new(x), Moo::new(y))),
        ),
        decisions,
        symbols,
    )
}

fn gate(
    expression: Expr,
    decisions: &mut Vec<SatEncodingDecision>,
    symbols: &mut SymbolTable,
) -> Expr {
    let output = create_bool_aux(symbols);
    decisions.push(SatEncodingDecision::Boolean {
        output: output.clone(),
        expression,
    });
    output
}

/// Move a top-level Boolean assertion into the SAT decision payload.
#[register_rule("SAT", 8400, [Root])]
fn remove_single_atom(expr: &Expr, symbols: &SymbolTable) -> ApplicationResult {
    // The single atom must not be within another expression
    let Expr::Root(_, children) = expr else {
        return Err(RuleNotApplicable);
    };

    // Find the position of the first reference atom with boolean domain
    let Some(pos) = children.iter().position(
        |e| matches!(e, Expr::Atomic(_, Atom::Reference(x)) if x.domain().is_some_and(|d| d.is_bool())),
    ) else {
        return Err(RuleNotApplicable);
    };

    // Clone the children since expr is borrowed immutably
    let mut new_children = children.clone();

    let removed = new_children.remove(pos);

    let new_sat_decisions = vec![SatEncodingDecision::Assert(removed)];

    // If now empty, replace with `true`
    if new_children.is_empty() {
        new_children.push(essence_expr!(true));
    }

    let new_expr = Expr::Root(Metadata::new(), new_children);

    Ok(RuleEffect::sat(
        new_expr,
        new_sat_decisions,
        symbols.clone(),
    ))
}

/// Lower the Boolean operation to a semantic SAT gate decision.
#[register_rule("SAT", 8500, [And, Or])]
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
    let mut new_sat_decisions = vec![];
    let mut new_symbols = symbols.clone();

    match expr {
        Expr::And(_, _) => {
            new_expr = tseytin_and(exprs_list, &mut new_sat_decisions, &mut new_symbols);
        }
        Expr::Or(_, _) => {
            new_expr = tseytin_or(exprs_list, &mut new_sat_decisions, &mut new_symbols);
        }
        _ => return Err(RuleNotApplicable),
    };

    Ok(RuleEffect::sat(new_expr, new_sat_decisions, new_symbols))
}

/// Lower the Boolean operation to a semantic SAT gate decision.
#[register_rule("SAT", 9005, [Not])]
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

    let mut new_sat_decisions = vec![];
    let mut new_symbols = symbols.clone();

    let new_expr = tseytin_not(x.as_ref().clone(), &mut new_sat_decisions, &mut new_symbols);

    Ok(RuleEffect::sat(new_expr, new_sat_decisions, new_symbols))
}

/// Lower the Boolean operation to a semantic SAT gate decision.
#[register_rule("SAT", 8500, [Iff, Eq])]
fn apply_tseytin_iff_eq(expr: &Expr, symbols: &SymbolTable) -> ApplicationResult {
    // Check for iff or eq
    let (x, y) = match expr {
        Expr::Iff(_, x, y) | Expr::Eq(_, x, y) => (x, y),
        _ => return Err(RuleNotApplicable),
    };

    if !is_literal(x.as_ref()) || !is_literal(y.as_ref()) {
        return Err(RuleNotApplicable);
    };

    let mut new_sat_decisions = vec![];
    let mut new_symbols = symbols.clone();

    let new_expr = tseytin_iff(
        x.as_ref().clone(),
        y.as_ref().clone(),
        &mut new_sat_decisions,
        &mut new_symbols,
    );

    Ok(RuleEffect::sat(new_expr, new_sat_decisions, new_symbols))
}

/// Lower the Boolean operation to a semantic SAT gate decision.
#[register_rule("SAT", 8500, [Imply])]
fn apply_tseytin_imply(expr: &Expr, symbols: &SymbolTable) -> ApplicationResult {
    let Expr::Imply(_, x, y) = expr else {
        return Err(RuleNotApplicable);
    };

    if !is_literal(x.as_ref()) || !is_literal(y.as_ref()) {
        return Err(RuleNotApplicable);
    };

    let new_expr;
    let mut new_sat_decisions = vec![];
    let mut new_symbols = symbols.clone();

    new_expr = tseytin_imply(
        x.as_ref().clone(),
        y.as_ref().clone(),
        &mut new_sat_decisions,
        &mut new_symbols,
    );

    Ok(RuleEffect::sat(new_expr, new_sat_decisions, new_symbols))
}

/// Lower the Boolean operation to a semantic SAT gate decision.
#[register_rule("SAT", 8500, [Neq])]
fn apply_tseytin_xor_neq(expr: &Expr, symbols: &SymbolTable) -> ApplicationResult {
    let Expr::Neq(_, x, y) = expr else {
        return Err(RuleNotApplicable);
    };

    if !is_literal(x.as_ref()) || !is_literal(y.as_ref()) {
        return Err(RuleNotApplicable);
    };

    let mut new_sat_decisions = vec![];
    let mut new_symbols = symbols.clone();

    let new_expr = tseytin_xor(
        x.as_ref().clone(),
        y.as_ref().clone(),
        &mut new_sat_decisions,
        &mut new_symbols,
    );

    Ok(RuleEffect::sat(new_expr, new_sat_decisions, new_symbols))
}
