use conjure_cp::essence_expr;

use conjure_cp::ast::Metadata;
use conjure_cp::ast::{Atom, Expression as Expr, Moo, SatEncodingDecision};
use conjure_cp::rule_engine::{
    ApplicationError::RuleNotApplicable, ApplicationResult, RuleEffect, register_rule,
};

use conjure_cp::ast::AbstractLiteral::Matrix;
use conjure_cp::ast::{Domain, SymbolTable};

// Atomic references can denote integers and compound values too. Those must reach their
// representation's comparison rules rather than becoming Boolean gate inputs.
fn is_literal(expr: &Expr) -> bool {
    crate::shared::utils::is_literal(expr)
        && expr.domain_of().is_some_and(|domain| domain.is_bool())
}

pub(super) fn create_bool_aux(symbols: &mut SymbolTable) -> Expr {
    let name = symbols.gen_find_auxiliary(&Domain::bool());

    symbols.insert(name.clone());

    Expr::Atomic(
        Metadata::new(),
        Atom::Reference(conjure_cp::ast::Reference::new(name)),
    )
}

/// Boolean order is false before true and shares the existing gate decisions.
#[register_rule("SAT", 18500, [Lt, Gt, Leq, Geq])]
fn boolean_order(expr: &Expr, _: &SymbolTable) -> ApplicationResult {
    let (left, right, strict) = match expr {
        Expr::Lt(_, left, right) => (left, right, true),
        Expr::Gt(_, left, right) => (right, left, true),
        Expr::Leq(_, left, right) => (left, right, false),
        Expr::Geq(_, left, right) => (right, left, false),
        _ => return Err(RuleNotApplicable),
    };
    if [left, right]
        .iter()
        .any(|operand| !operand.domain_of().is_some_and(|domain| domain.is_bool()))
    {
        return Err(RuleNotApplicable);
    }
    Ok(RuleEffect::pure(if strict {
        Expr::And(
            Metadata::new(),
            Moo::new(conjure_cp::into_matrix_expr!(vec![
                Expr::Not(Metadata::new(), left.clone()),
                (**right).clone(),
            ])),
        )
    } else {
        Expr::Imply(Metadata::new(), left.clone(), right.clone())
    }))
}

/// Record an AND gate without generating clauses.
pub fn tseytin_and(
    exprs: &[Expr],
    decisions: &mut Vec<SatEncodingDecision>,
    symbols: &mut SymbolTable,
) -> Expr {
    gate(
        Expr::And(
            Metadata::new(),
            Moo::new(conjure_cp::into_matrix_expr!(exprs.to_vec())),
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
    exprs: &[Expr],
    decisions: &mut Vec<SatEncodingDecision>,
    symbols: &mut SymbolTable,
) -> Expr {
    gate(
        Expr::Or(
            Metadata::new(),
            Moo::new(conjure_cp::into_matrix_expr!(exprs.to_vec())),
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

/// Shared recognition keeps asserted Boolean counts in their own encoding family.
pub(super) fn count_inputs(sum: &Expr) -> Option<Vec<Expr>> {
    let Expr::AbstractLiteral(_, Matrix(terms, _)) = sum else {
        return None;
    };
    terms
        .iter()
        .map(|term| match term {
            Expr::ToInt(_, input)
                if is_literal(input)
                    && input.domain_of().is_some_and(|domain| domain.is_bool()) =>
            {
                Some(input.as_ref().clone())
            }
            Expr::Atomic(_, Atom::Literal(conjure_cp::ast::Literal::Int(value)))
                if *value == 0 || *value == 1 =>
            {
                Some((*value == 1).into())
            }
            _ => None,
        })
        .collect::<Option<Vec<_>>>()
}

/// Preserve asserted Boolean counts until a library encoder is selected.
#[register_rule("SAT", 20000, [Root])]
fn select_cardinality(expr: &Expr, symbols: &SymbolTable) -> ApplicationResult {
    use conjure_cp::ast::sat_decision::CardinalityRelation;
    super::asserted::select_asserted(expr, symbols, |child| {
        let (left, right, mut relation) = match child {
            Expr::Leq(_, left, right) => (left, right, CardinalityRelation::AtMost),
            Expr::Geq(_, left, right) => (left, right, CardinalityRelation::AtLeast),
            Expr::Eq(_, left, right) => (left, right, CardinalityRelation::Exactly),
            _ => return None,
        };
        let (sum, bound) = match (left.as_ref(), right.as_ref()) {
            (
                Expr::Sum(_, terms),
                Expr::Atomic(_, Atom::Literal(conjure_cp::ast::Literal::Int(bound))),
            ) => (terms, *bound),
            (
                Expr::Atomic(_, Atom::Literal(conjure_cp::ast::Literal::Int(bound))),
                Expr::Sum(_, terms),
            ) => {
                relation = match relation {
                    CardinalityRelation::AtMost => CardinalityRelation::AtLeast,
                    CardinalityRelation::AtLeast => CardinalityRelation::AtMost,
                    other => other,
                };
                (terms, *bound)
            }
            _ => return None,
        };
        let inputs = count_inputs(sum)?;
        Some(if relation == CardinalityRelation::AtMost && bound == 1 {
            SatEncodingDecision::AtMostOne {
                inputs,
                encoding: None,
            }
        } else {
            SatEncodingDecision::Cardinality {
                inputs,
                relation,
                bound: i64::from(bound),
                encoding: None,
            }
        })
    })
}

#[cfg(test)]
mod amo_lowering_tests {
    use super::*;
    #[test]
    fn boolean_order_matches_all_truth_assignments_and_declines_integers() {
        for left in [false, true] {
            for right in [false, true] {
                let lhs = Moo::new(Expr::from(left));
                let rhs = Moo::new(Expr::from(right));
                for (expr, expected) in [
                    (
                        Expr::Lt(Metadata::new(), lhs.clone(), rhs.clone()),
                        i32::from(left) < i32::from(right),
                    ),
                    (
                        Expr::Gt(Metadata::new(), lhs.clone(), rhs.clone()),
                        i32::from(left) > i32::from(right),
                    ),
                    (
                        Expr::Leq(Metadata::new(), lhs.clone(), rhs.clone()),
                        i32::from(left) <= i32::from(right),
                    ),
                    (
                        Expr::Geq(Metadata::new(), lhs.clone(), rhs.clone()),
                        i32::from(left) >= i32::from(right),
                    ),
                ] {
                    let lowered = boolean_order(&expr, &SymbolTable::new()).unwrap();
                    assert_eq!(
                        conjure_cp::ast::eval_constant(&lowered.new_expression),
                        Some(expected.into())
                    );
                }
            }
        }
        let numeric = Expr::Lt(Metadata::new(), Moo::new(0.into()), Moo::new(1.into()));
        assert!(boolean_order(&numeric, &SymbolTable::new()).is_err());
    }
    #[test]
    fn cardinality_accepts_simplified_constants_and_reversed_bounds() {
        use conjure_cp::ast::sat_decision::CardinalityRelation;
        let sum = Expr::Sum(
            Metadata::new(),
            Moo::new(conjure_cp::into_matrix_expr!(vec![1.into(), 0.into()])),
        );
        let comparison = Expr::Leq(Metadata::new(), Moo::new(2.into()), Moo::new(sum));
        let effect = select_cardinality(
            &Expr::Root(Metadata::new(), vec![comparison]),
            &SymbolTable::new(),
        )
        .unwrap();
        assert!(
            matches!(&effect.new_sat_decisions[0], SatEncodingDecision::Cardinality {
            inputs, relation: CardinalityRelation::AtLeast, bound: 2, ..
        } if inputs == &vec![true.into(), false.into()])
        );
    }
    #[test]
    fn amo_waits_until_boolean_operands_have_been_lowered() {
        let complex = Expr::Eq(Metadata::new(), Moo::new(2.into()), Moo::new(3.into()));
        let indicator = Expr::ToInt(Metadata::new(), Moo::new(complex));
        let sum = Expr::Sum(
            Metadata::new(),
            Moo::new(conjure_cp::into_matrix_expr!(vec![indicator])),
        );
        let cardinality = Expr::Leq(Metadata::new(), Moo::new(sum), Moo::new(1.into()));
        let root = Expr::Root(Metadata::new(), vec![cardinality]);
        assert!(select_cardinality(&root, &SymbolTable::new()).is_err());
    }

    #[test]
    fn gates_wait_for_integer_and_matrix_comparisons() {
        let symbols = SymbolTable::new();
        for domain in [
            Domain::int_ground(vec![conjure_cp::ast::Range::Bounded(1, 3)]),
            Domain::matrix(
                Domain::bool(),
                vec![Domain::int_ground(vec![conjure_cp::ast::Range::Bounded(
                    1, 3,
                )])],
            ),
        ] {
            let left: Expr =
                conjure_cp::ast::Reference::new(symbols.clone().gen_find(&domain)).into();
            let right: Expr =
                conjure_cp::ast::Reference::new(symbols.clone().gen_find(&domain)).into();
            let expr = Expr::Eq(Metadata::new(), Moo::new(left), Moo::new(right));
            assert!(apply_tseytin_iff_eq(&expr, &symbols).is_err());
        }
    }
}
