use conjure_cp::ast::{Atom, Expression as Expr, SATIntEncoding, SymbolTable};
use conjure_cp::rule_engine::{
    ApplicationError, ApplicationError::RuleNotApplicable, ApplicationResult, RuleEffect,
    register_rule,
};

use crate::types::int::IntLog;

/// Normalise logarithmic SAT integer operands to a common bit width.
pub(super) fn validate_log_int_operands(
    exprs: Vec<Expr>,
    bit_count: Option<u32>,
) -> Result<Vec<Vec<Expr>>, ApplicationError> {
    let mut out: Vec<Vec<Expr>> = exprs
        .into_iter()
        .map(|expr| match expr {
            Expr::SATInt(_, SATIntEncoding::Log, inner, _) => inner
                .as_ref()
                .clone()
                .unwrap_list()
                .ok_or(RuleNotApplicable),
            _ => Err(RuleNotApplicable),
        })
        .collect::<Result<_, _>>()?;

    let max_len = bit_count
        .map(|bits| bits as usize)
        .unwrap_or_else(|| out.iter().map(Vec::len).max().unwrap_or(0));
    for bits in &mut out {
        if bits.len() < max_len {
            if let Some(last) = bits.last().cloned() {
                bits.resize(max_len, last);
            }
        } else {
            bits.truncate(max_len);
        }
    }
    Ok(out)
}

/// Replace a log-represented reference with its bit vector.
#[register_rule("SAT", 9500, [Atomic])]
fn integer_decision_representation_log(expr: &Expr, _: &SymbolTable) -> ApplicationResult {
    let Expr::Atomic(_, Atom::Reference(reference)) = expr else {
        return Err(RuleNotApplicable);
    };
    let state = reference.get_repr_as::<IntLog>().ok_or(RuleNotApplicable)?;
    Ok(RuleEffect::pure(state.sat_int_expr()))
}

/// Number of bits required to encode an `i32` in two's complement.
pub(in crate::backends::sat) fn bit_magnitude(value: i32) -> usize {
    if value >= 0 {
        (1 + (32 - value.leading_zeros())).try_into().unwrap()
    } else {
        (33 - (!value).leading_zeros()).try_into().unwrap()
    }
}

/// Sign-extend the shorter of two bit vectors to the length of the longer one.
pub(super) fn match_bits_length(mut lhs: Vec<Expr>, mut rhs: Vec<Expr>) -> (Vec<Expr>, Vec<Expr>) {
    if lhs.len() < rhs.len() {
        lhs.resize(rhs.len(), lhs.last().cloned().unwrap());
    } else if rhs.len() < lhs.len() {
        rhs.resize(lhs.len(), rhs.last().cloned().unwrap());
    }
    (lhs, rhs)
}
