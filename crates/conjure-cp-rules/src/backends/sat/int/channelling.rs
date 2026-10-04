//! Channelling between the SAT integer encodings.
//!
//! Which encoding an integer gets is a per-declaration representation choice, so one constraint
//! can end up comparing a one-hot variable with a bit-vector one. The encoding-specific rules all
//! decline in that case -- each only knows how to read its own layout -- so a mixed operation is
//! first rewritten to put every operand in the same encoding.
//!
//! Fallback operations use actual-value binary circuits. Direct and Order supply value
//! indicators; Offset adds the domain minimum and Rank maps canonical intervals to their
//! numeric values. These temporary circuit operands do not create additional representations.

use std::collections::{HashSet, VecDeque};

use conjure_cp::ast::{
    AbstractLiteral, Atom, DomainPtr, Expression as Expr, Literal, Metadata, Moo, SATIntEncoding,
    SatEncodingDecision, SymbolTable,
};
use conjure_cp::into_matrix_expr;
use conjure_cp::rule_engine::{
    ApplicationError::RuleNotApplicable, ApplicationResult, RuleEffect, register_rule,
};

use crate::backends::sat::boolean::{
    tseytin_and, tseytin_mux, tseytin_not, tseytin_or, tseytin_xor,
};
use crate::backends::sat::int::log::bit_magnitude;
use uniplate::Uniplate;

/// Put every `SATInt` operand of an operation into the logarithmic encoding.
///
/// This is a fallback, and sits below every encoding-specific operation rule on purpose. An
/// operation whose operands share an encoding that knows how to encode it is handled there and
/// never reaches this rule. Remaining operands may have different encodings, use an encoding with
/// no rule for the operation, or include Boolean indicators needing an actual-value circuit view.
#[register_rule("SAT", 4000, [Eq, Neq, Lt, Gt, Leq, Geq, AllDiff, Table, NegativeTable, SatElement, SatObjective, Sum, Product, Min, Max, Abs, Neg, SafeDiv, SafeMod, SafePow])]
fn unify_sat_int_encodings(expr: &Expr, symbols: &SymbolTable) -> ApplicationResult {
    let mut encodings = HashSet::new();
    let mut has_indicator = false;
    for operand in operands(expr) {
        has_indicator |= ready_indicator(&operand);
        if let Expr::SATInt(_, encoding, _, _) = operand {
            encodings.insert(encoding);
        }
    }

    // Semantic allDifferent/table/element operands retain their ready value views.
    // Only rank codes need conversion; the decision rule handles the other encodings.
    let retains_views = matches!(
        expr,
        Expr::AllDiff(..)
            | Expr::Table(..)
            | Expr::NegativeTable(..)
            | Expr::SatElement(..)
            | Expr::SatObjective(..)
    );
    if retains_views
        && !encodings
            .iter()
            .any(|encoding| matches!(encoding, SATIntEncoding::Rank(_)))
    {
        return Err(RuleNotApplicable);
    }
    // Ready indicators need an actual-value view only when a circuit is required.
    // Linear and count decisions run earlier and retain their original inputs.
    let has_indicator = !retains_views && has_indicator;
    if !has_indicator
        && encodings
            .iter()
            .all(|encoding| matches!(encoding, SATIntEncoding::Log))
    {
        return Err(RuleNotApplicable);
    }

    let mut clauses = Vec::new();
    let mut new_symbols = symbols.clone();

    let mut convert = |input: Expr| {
        if retains_views && !matches!(&input, Expr::SATInt(_, SATIntEncoding::Rank(_), _, _)) {
            input
        } else {
            to_log(input, &mut clauses, &mut new_symbols)
        }
    };
    let children: VecDeque<Expr> = expr
        .children()
        .into_iter()
        .map(|child| match matrix_child(&child) {
            Some((elements, index_domain)) => {
                let converted: Vec<Expr> = elements.into_iter().map(&mut convert).collect();
                rebuild_matrix_child(converted, index_domain)
            }
            None => convert(child),
        })
        .collect();

    Ok(RuleEffect::sat(
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
fn to_log(expr: Expr, clauses: &mut Vec<SatEncodingDecision>, symbols: &mut SymbolTable) -> Expr {
    if ready_indicator(&expr) {
        let Expr::ToInt(_, input) = expr else {
            unreachable!()
        };
        return Expr::SATInt(
            Metadata::new(),
            SATIntEncoding::Log,
            Moo::new(into_matrix_expr!(vec![
                input.as_ref().clone(),
                false.into()
            ])),
            (0, 1),
        );
    }
    let Expr::SATInt(_, encoding, bits, bounds) = &expr else {
        return expr;
    };
    if matches!(encoding, SATIntEncoding::Log) {
        return expr;
    }
    let Some(bits) = bits.as_ref().clone().into_list() else {
        return expr;
    };

    let (low, high) = *bounds;
    let width = bit_magnitude(low).max(bit_magnitude(high));
    if matches!(encoding, SATIntEncoding::Offset | SATIntEncoding::Rank(_)) {
        let mut decoded = add_unsigned_constant(&bits, i64::from(low), width, clauses, symbols);
        if let SATIntEncoding::Rank(ranges) = encoding {
            let mut start = 0u64;
            for (index, &(range_low, range_high)) in ranges.iter().enumerate() {
                if index > 0 {
                    let selector = unsigned_at_least(&bits, start, clauses, symbols);
                    let candidate = add_unsigned_constant(
                        &bits,
                        i64::from(range_low) - start as i64,
                        width,
                        clauses,
                        symbols,
                    );
                    decoded = decoded
                        .into_iter()
                        .zip(candidate)
                        .map(|(old, new)| tseytin_mux(selector.clone(), old, new, clauses, symbols))
                        .collect();
                }
                start += (i64::from(range_high) - i64::from(range_low) + 1) as u64;
            }
        }
        return Expr::SATInt(
            Metadata::new(),
            SATIntEncoding::Log,
            Moo::new(into_matrix_expr!(decoded)),
            (low, high),
        );
    }

    // Both remaining encodings lay out one bit per value; order does so cumulatively, so take the
    // difference between neighbouring thresholds to recover "x is exactly this value".
    let value_bits = match encoding {
        SATIntEncoding::Direct => bits,
        SATIntEncoding::Order => order_to_value_bits(&bits, clauses, symbols),
        SATIntEncoding::Log | SATIntEncoding::Offset | SATIntEncoding::Rank(_) => return expr,
    };

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

fn ready_indicator(expr: &Expr) -> bool {
    matches!(expr, Expr::ToInt(_, input)
        if crate::shared::utils::is_literal(input)
            && input.domain_of().is_some_and(|domain| domain.is_bool()))
}

/// Turn order-encoded thresholds into one bit per value.
///
/// `x = low + i` exactly when `x >= low + i` holds and `x >= low + i + 1` does not; past the top
/// threshold there is nothing left to exclude.
fn order_to_value_bits(
    thresholds: &[Expr],
    clauses: &mut Vec<SatEncodingDecision>,
    symbols: &mut SymbolTable,
) -> Vec<Expr> {
    thresholds
        .iter()
        .enumerate()
        .map(|(index, threshold)| match thresholds.get(index + 1) {
            Some(next) => {
                let not_next = tseytin_not(next.clone(), clauses, symbols);
                tseytin_and(&[threshold.clone(), not_next], clauses, symbols)
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
        SATIntEncoding::Offset | SATIntEncoding::Rank(_) => vec![false.into()],
        SATIntEncoding::Direct | SATIntEncoding::Order => {
            vec![Expr::Atomic(
                Metadata::new(),
                Atom::Literal(Literal::Bool(true)),
            )]
        }
    };

    Expr::SATInt(
        Metadata::new(),
        match encoding {
            SATIntEncoding::Rank(_) => SATIntEncoding::Rank(vec![(value, value)]),
            other => other.clone(),
        },
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

/// Add a constant modulo the semantic width; structural constraints exclude unused codes.
fn add_unsigned_constant(
    bits: &[Expr],
    constant: i64,
    width: usize,
    decisions: &mut Vec<SatEncodingDecision>,
    symbols: &mut SymbolTable,
) -> Vec<Expr> {
    let mut carry: Expr = false.into();
    let mut out = Vec::with_capacity(width);
    for index in 0..width {
        let bit = bits.get(index).cloned().unwrap_or_else(|| false.into());
        let sum = tseytin_xor(bit.clone(), carry.clone(), decisions, symbols);
        if (constant as u32 >> index) & 1 == 1 {
            out.push(tseytin_not(sum, decisions, symbols));
            carry = tseytin_or(&[bit, carry], decisions, symbols);
        } else {
            out.push(sum);
            carry = tseytin_and(&[bit, carry], decisions, symbols);
        }
    }
    out
}
/// Compare an unsigned code with an interval's starting rank.
fn unsigned_at_least(
    bits: &[Expr],
    minimum: u64,
    decisions: &mut Vec<SatEncodingDecision>,
    symbols: &mut SymbolTable,
) -> Expr {
    let maximum = minimum - 1;
    let mut leq: Expr = true.into();
    for (index, bit) in bits.iter().enumerate() {
        let not = tseytin_not(bit.clone(), decisions, symbols);
        leq = if (maximum >> index) & 1 == 1 {
            tseytin_or(&[not, leq], decisions, symbols)
        } else {
            tseytin_and(&[not, leq], decisions, symbols)
        };
    }
    tseytin_not(leq, decisions, symbols)
}

#[cfg(test)]
mod unsigned_tests {
    use super::*;
    use crate::types::int::unsigned::{unsigned_capacity, unsigned_width, value_at_rank};
    use conjure_cp::ast::Name;
    use std::collections::HashMap;

    #[test]
    fn indicator_circuit_view_has_zero_sign_bit_without_auxiliaries() {
        for value in [false, true] {
            let indicator = Expr::ToInt(Metadata::new(), Moo::new(value.into()));
            let mut decisions = Vec::new();
            let decoded = to_log(indicator, &mut decisions, &mut SymbolTable::new());
            let Expr::SATInt(_, SATIntEncoding::Log, bits, (0, 1)) = decoded else {
                panic!("expected an actual-value indicator view")
            };
            let bits = bits.unwrap_list_ref().unwrap();
            assert_eq!(bits.len(), 2);
            assert_eq!(bits[0], value.into());
            assert_eq!(bits[1], false.into());
            assert!(decisions.is_empty());
        }
    }

    #[test]
    fn alldifferent_waits_for_value_views_and_only_converts_rank_codes() {
        use conjure_cp::ast::{DeclarationPtr, Domain, Reference};
        let bit: Expr = Reference::new(DeclarationPtr::new_find(
            Name::User("choice".into()),
            Domain::bool(),
        ))
        .into();
        let pending: Expr = Reference::new(DeclarationPtr::new_find(
            Name::User("pending".into()),
            Domain::int(vec![conjure_cp::ast::Range::Bounded(1, 3)]),
        ))
        .into();
        let direct = Expr::SATInt(
            Metadata::new(),
            SATIntEncoding::Direct,
            Moo::new(into_matrix_expr!(vec![bit.clone()])),
            (0, 0),
        );
        let build = |inputs| Expr::AllDiff(Metadata::new(), Moo::new(into_matrix_expr!(inputs)));
        assert!(
            unify_sat_int_encodings(
                &build(vec![direct.clone(), pending.clone()]),
                &SymbolTable::new()
            )
            .is_err()
        );
        let rank = Expr::SATInt(
            Metadata::new(),
            SATIntEncoding::Rank(vec![(1, 1), (3, 3)]),
            Moo::new(into_matrix_expr!(vec![bit])),
            (1, 3),
        );
        let effect =
            unify_sat_int_encodings(&build(vec![direct, rank, pending]), &SymbolTable::new())
                .unwrap();
        let Expr::AllDiff(_, matrix) = effect.new_expression else {
            panic!("allDifferent must remain semantic")
        };
        let inputs = matrix.unwrap_list_cow().unwrap();
        assert!(matches!(
            &inputs[0],
            Expr::SATInt(_, SATIntEncoding::Direct, _, _)
        ));
        assert!(matches!(
            &inputs[1],
            Expr::SATInt(_, SATIntEncoding::Log, _, _)
        ));
        assert!(matches!(&inputs[2], Expr::Atomic(_, Atom::Reference(_))));
    }

    fn evaluate(expr: &Expr, values: &HashMap<Name, bool>) -> bool {
        match expr {
            Expr::Atomic(_, Atom::Literal(Literal::Bool(value))) => *value,
            Expr::Atomic(_, Atom::Reference(reference)) => values[&reference.name()],
            Expr::Not(_, input) => !evaluate(input, values),
            Expr::Iff(_, left, right) => evaluate(left, values) == evaluate(right, values),
            Expr::And(_, inner) => inner
                .unwrap_list_ref()
                .unwrap()
                .iter()
                .all(|x| evaluate(x, values)),
            Expr::Or(_, inner) => inner
                .unwrap_list_ref()
                .unwrap()
                .iter()
                .any(|x| evaluate(x, values)),
            other => panic!("unexpected Boolean gate: {other}"),
        }
    }

    #[test]
    fn unused_unsigned_codes_are_excluded_even_for_singletons() {
        use crate::types::int::unsigned::unsigned_bound;
        use conjure_cp::ast::{DeclarationPtr, Domain};
        for maximum in [0, 1, 2, 5, 7, 8, 17, u32::MAX as u64] {
            let bits: Vec<_> = (0..unsigned_width(maximum))
                .map(|i| {
                    DeclarationPtr::new_find(Name::User(format!("b{i}").into()), Domain::bool())
                })
                .collect();
            let bound = unsigned_bound(&bits, maximum);
            let codes: Vec<u64> = if maximum < 32 {
                (0..1u64 << bits.len()).collect()
            } else {
                vec![0, 1, 1 << 31, u32::MAX as u64]
            };
            for code in codes {
                let values: HashMap<_, _> = bits
                    .iter()
                    .enumerate()
                    .map(|(i, decl)| (decl.name().clone(), (code >> i) & 1 != 0))
                    .collect();
                assert_eq!(evaluate(&bound, &values), code <= maximum);
            }
        }
    }

    #[test]
    fn unsigned_codes_decode_to_actual_values_including_sparse_and_extreme_domains() {
        let domains = [
            vec![(-3, -3), (-1, -1), (2, 2)],
            vec![(-8, -6), (3, 5)],
            vec![(100, 105)],
            vec![(-100, -93)],
            vec![(-4, 3)],
            vec![(i32::MIN, i32::MIN)],
            vec![(i32::MAX, i32::MAX)],
            vec![(i32::MIN, i32::MAX)],
            vec![(i32::MIN, i32::MIN), (i32::MAX, i32::MAX)],
        ];
        for ranges in domains {
            let low = ranges[0].0;
            let high = ranges.last().unwrap().1;
            for rank in [false, true] {
                let capacity = unsigned_capacity(&ranges, rank);
                let codes: Vec<u64> = if capacity <= 128 {
                    (0..capacity).collect()
                } else {
                    vec![0, 1, capacity / 2, capacity - 2, capacity - 1]
                };
                for code in codes {
                    let expected = if rank {
                        i64::from(value_at_rank(&ranges, code).unwrap())
                    } else {
                        i64::from(low) + code as i64
                    };
                    let bits: Vec<Expr> = (0..unsigned_width(capacity - 1))
                        .map(|i| ((code >> i) & 1 == 1).into())
                        .collect();
                    let encoding = if rank {
                        SATIntEncoding::Rank(ranges.clone())
                    } else {
                        SATIntEncoding::Offset
                    };
                    let operand = Expr::SATInt(
                        Metadata::new(),
                        encoding,
                        Moo::new(into_matrix_expr!(bits)),
                        (low, high),
                    );
                    let mut decisions = Vec::new();
                    let decoded = to_log(operand, &mut decisions, &mut SymbolTable::new());
                    let mut values = HashMap::new();
                    for decision in decisions {
                        let SatEncodingDecision::Boolean { output, expression } = decision else {
                            panic!("unexpected decision")
                        };
                        let Expr::Atomic(_, Atom::Reference(reference)) = output else {
                            panic!("unexpected output")
                        };
                        values.insert(reference.name().clone(), evaluate(&expression, &values));
                    }
                    let Expr::SATInt(_, SATIntEncoding::Log, inner, _) = decoded else {
                        panic!("expected actual-value binary")
                    };
                    let bits = inner.unwrap_list_ref().unwrap();
                    let width = bits.len();
                    let mut value: i64 = bits
                        .iter()
                        .enumerate()
                        .map(|(i, bit)| i64::from(evaluate(bit, &values)) << i)
                        .sum();
                    if value & (1i64 << (width - 1)) != 0 {
                        value -= 1i64 << width;
                    }
                    assert_eq!(value, expected, "rank={rank} ranges={ranges:?} code={code}");
                }
            }
        }
    }
}
