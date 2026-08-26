//! Channelling between the SAT integer encodings.
//!
//! Which encoding an integer gets is a per-declaration representation choice, so one constraint
//! can end up comparing a one-hot variable with a bit-vector one. The encoding-specific rules all
//! decline in that case -- each only knows how to read its own layout -- so a mixed operation is
//! first rewritten to put every operand in the same encoding.
//!
//! Everything is channelled into the logarithmic encoding. Reaching it from the other two is a
//! disjunction per bit, whereas going the other way costs a variable per value of the operand's
//! range, and the log encoding is the one whose width does not grow with the domain.

use std::collections::{HashSet, VecDeque};

use conjure_cp::ast::{
    AbstractLiteral, Atom, CnfClause, DomainPtr, Expression as Expr, Literal, Metadata, Moo,
    SATIntEncoding, SymbolTable,
};
use conjure_cp::into_matrix_expr;
use conjure_cp::rule_engine::{
    ApplicationError::RuleNotApplicable, ApplicationResult, RuleEffect, register_rule,
};

use crate::backends::sat::boolean::{tseytin_and, tseytin_not, tseytin_or};
use crate::backends::sat::int::log::bit_magnitude;
use uniplate::Uniplate;

/// Put every `SATInt` operand of an operation into the logarithmic encoding.
///
/// This is a fallback, and sits below every encoding-specific operation rule on purpose. An
/// operation whose operands share an encoding that knows how to encode it is handled there and
/// never reaches this rule; what is left over is the two cases that would otherwise be stuck --
/// operands in different encodings, and an operation the operands' shared encoding has no rule
/// for, such as a sum of order-encoded variables.
#[register_rule("SAT", 4000, [Eq, Neq, Lt, Gt, Leq, Geq, Sum, Product, Min, Max, Abs, Neg, SafeDiv, SafeMod, SafePow])]
fn unify_sat_int_encodings(expr: &Expr, symbols: &SymbolTable) -> ApplicationResult {
    let encodings: HashSet<SATIntEncoding> = operands(expr)
        .filter_map(|operand| match operand {
            Expr::SATInt(_, encoding, _, _) => Some(encoding),
            _ => None,
        })
        .collect();

    // Nothing to do when every operand is already logarithmic, which includes having no `SATInt`
    // operands at all.
    if encodings
        .iter()
        .all(|encoding| matches!(encoding, SATIntEncoding::Log))
    {
        return Err(RuleNotApplicable);
    }

    let mut clauses = Vec::new();
    let mut new_symbols = symbols.clone();

    let children: VecDeque<Expr> = expr
        .children()
        .into_iter()
        .map(|child| match matrix_child(&child) {
            Some((elements, index_domain)) => {
                let converted: Vec<Expr> = elements
                    .into_iter()
                    .map(|element| to_log(element, &mut clauses, &mut new_symbols))
                    .collect();
                rebuild_matrix_child(converted, index_domain)
            }
            None => to_log(child, &mut clauses, &mut new_symbols),
        })
        .collect();

    Ok(RuleEffect::cnf(
        expr.with_children(children),
        clauses,
        new_symbols,
    ))
}

/// The elements of a matrix-literal child, with the index domain to rebuild it under.
///
/// `unwrap_list` only recognises the implied `int(1..)` index domain, but a matrix rebuilt from its
/// components keeps the one it was declared with. These rules look through a child to reach the
/// operands underneath, and an operand is no less an operand for being indexed from zero.
fn matrix_child(expr: &Expr) -> Option<(Vec<Expr>, DomainPtr)> {
    match expr {
        Expr::AbstractLiteral(_, AbstractLiteral::Matrix(elements, index_domain)) => {
            Some((elements.clone(), index_domain.clone()))
        }
        _ => None,
    }
}

/// Rebuild a matrix-literal child from new elements, keeping its index domain.
fn rebuild_matrix_child(elements: Vec<Expr>, index_domain: DomainPtr) -> Expr {
    Expr::AbstractLiteral(
        Metadata::new(),
        AbstractLiteral::Matrix(elements, index_domain),
    )
}

/// The operand expressions of `expr`, looking one level into list children.
///
/// `Sum` and friends hold their operands in a matrix child rather than directly, so a mixed
/// summation would otherwise look uniform from the outside.
fn operands(expr: &Expr) -> impl Iterator<Item = Expr> {
    expr.children()
        .into_iter()
        .flat_map(|child| match matrix_child(&child) {
            Some((elements, _)) => elements,
            None => vec![child],
        })
}

/// Re-encode one operand into the logarithmic encoding, leaving anything else alone.
fn to_log(expr: Expr, clauses: &mut Vec<CnfClause>, symbols: &mut SymbolTable) -> Expr {
    let Expr::SATInt(_, encoding, bits, bounds) = &expr else {
        return expr;
    };
    if matches!(encoding, SATIntEncoding::Log) {
        return expr;
    }
    let Some(bits) = bits.as_ref().clone().unwrap_list() else {
        return expr;
    };

    // Both remaining encodings lay out one bit per value; order does so cumulatively, so take the
    // difference between neighbouring thresholds to recover "x is exactly this value".
    let value_bits = match encoding {
        SATIntEncoding::Direct => bits,
        SATIntEncoding::Order => order_to_value_bits(&bits, clauses, symbols),
        SATIntEncoding::Log => return expr,
    };

    let (low, high) = *bounds;
    let width = bit_magnitude(low).max(bit_magnitude(high));
    let log_bits: Vec<Expr> = (0..width)
        .map(|index| {
            // Bit `index` of `x` is set exactly when `x` takes one of the values whose two's
            // complement has that bit set.
            let terms: Vec<Expr> = value_bits
                .iter()
                .enumerate()
                .filter(|(offset, _)| ((low + *offset as i32) as u32) >> index & 1 == 1)
                .map(|(_, bit)| bit.clone())
                .collect();
            match terms.len() {
                0 => Expr::Atomic(Metadata::new(), Atom::Literal(Literal::Bool(false))),
                1 => terms.into_iter().next().expect("just checked the length"),
                _ => tseytin_or(&terms, clauses, symbols),
            }
        })
        .collect();

    Expr::SATInt(
        Metadata::new(),
        SATIntEncoding::Log,
        Moo::new(into_matrix_expr!(log_bits)),
        (low, high),
    )
}

/// Turn order-encoded thresholds into one bit per value.
///
/// `x = low + i` exactly when `x >= low + i` holds and `x >= low + i + 1` does not; past the top
/// threshold there is nothing left to exclude.
fn order_to_value_bits(
    thresholds: &[Expr],
    clauses: &mut Vec<CnfClause>,
    symbols: &mut SymbolTable,
) -> Vec<Expr> {
    thresholds
        .iter()
        .enumerate()
        .map(|(index, threshold)| match thresholds.get(index + 1) {
            Some(next) => {
                let not_next = tseytin_not(next.clone(), clauses, symbols);
                tseytin_and(&vec![threshold.clone(), not_next], clauses, symbols)
            }
            None => threshold.clone(),
        })
        .collect()
}

/// A constant in a given SAT integer encoding, with its value range pinned to the constant.
///
/// The direct and order encodings both need only a single set bit here: the padding in their
/// operand validators widens it to the range the operation works over.
pub(super) fn sat_int_literal(encoding: &SATIntEncoding, value: i32) -> Expr {
    let bits = match encoding {
        SATIntEncoding::Log => log_literal_bits(value),
        SATIntEncoding::Direct | SATIntEncoding::Order => {
            vec![Expr::Atomic(
                Metadata::new(),
                Atom::Literal(Literal::Bool(true)),
            )]
        }
    };

    Expr::SATInt(
        Metadata::new(),
        encoding.clone(),
        Moo::new(into_matrix_expr!(bits)),
        (value, value),
    )
}

/// The two's-complement bits of a constant, least significant first.
fn log_literal_bits(value: i32) -> Vec<Expr> {
    let mut remaining = value as u32;
    (0..bit_magnitude(value))
        .map(|_| {
            let bit = Expr::Atomic(
                Metadata::new(),
                Atom::Literal(Literal::Bool(remaining & 1 != 0)),
            );
            remaining >>= 1;
            bit
        })
        .collect()
}

/// Encode integer constants into whichever encoding the operation's variables are in.
///
/// Constants have no representation of their own to follow, and encoding one eagerly would mean
/// guessing: the same `2` belongs in a bit vector next to a log-encoded variable and in a one-hot
/// vector next to a direct-encoded one. Deferring until the operation is known keeps the encodings
/// separate, and leaves every operation rule below with the invariant it relies on -- that its
/// integer operands are all `SATInt`s.
#[register_rule("SAT", 9400, [Eq, Neq, Lt, Gt, Leq, Geq, Sum, Product, Min, Max, SafeDiv, SafeMod, SafePow])]
fn encode_sat_int_literals(expr: &Expr, _: &SymbolTable) -> ApplicationResult {
    let mut encoding = None;
    let mut has_literal = false;
    for operand in operands(expr) {
        match operand {
            Expr::SATInt(_, found, _, _) => encoding = encoding.or(Some(found)),
            Expr::Atomic(_, Atom::Literal(Literal::Int(_))) => has_literal = true,
            _ => {}
        }
    }

    // Without a variable to follow there is nothing to match, and without a constant there is
    // nothing to do.
    let (Some(encoding), true) = (encoding, has_literal) else {
        return Err(RuleNotApplicable);
    };

    let encode = |expr: Expr| match expr {
        Expr::Atomic(_, Atom::Literal(Literal::Int(value))) => sat_int_literal(&encoding, value),
        other => other,
    };

    let children: VecDeque<Expr> = expr
        .children()
        .into_iter()
        .map(|child| match matrix_child(&child) {
            Some((elements, index_domain)) => {
                rebuild_matrix_child(elements.into_iter().map(encode).collect(), index_domain)
            }
            None => encode(child),
        })
        .collect();

    Ok(RuleEffect::pure(expr.with_children(children)))
}
