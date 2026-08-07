use crate::backends::sat::integer::{defer_integer_representation, int_domain_to_expr};
use conjure_cp::ast::{
    Atom, Expression as Expr, GroundDomain, Literal, Metadata, Moo, SATIntEncoding, SymbolTable,
};
use conjure_cp::into_matrix_expr;
use conjure_cp::rule_engine::{
    ApplicationError, ApplicationError::RuleNotApplicable, ApplicationResult, RuleEffect,
    register_rule,
};

/// Normalise logarithmic SAT integer operands to a common bit width.
pub(super) fn validate_log_int_operands(
    exprs: Vec<Expr>,
    bit_count: Option<u32>,
) -> Result<Vec<Vec<Expr>>, ApplicationError> {
    let mut out: Vec<Vec<Expr>> = exprs
        .into_iter()
        .map(|expr| {
            let Expr::SATInt(_, SATIntEncoding::Log, inner, _) = expr else {
                return Err(RuleNotApplicable);
            };
            inner
                .as_ref()
                .clone()
                .unwrap_list()
                .ok_or(RuleNotApplicable)
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

#[register_rule("SAT_Log", 9500, [Atomic])]
fn integer_decision_representation_log(expr: &Expr, _: &SymbolTable) -> ApplicationResult {
    defer_integer_representation(expr, materialise_integer_decision_representation_log)
}

fn materialise_integer_decision_representation_log(
    expr: &Expr,
    symbols: &SymbolTable,
) -> ApplicationResult {
    let Expr::Atomic(_, Atom::Reference(name)) = expr else {
        return Err(RuleNotApplicable);
    };

    let dom = name.resolved_domain().ok_or(RuleNotApplicable)?;
    let GroundDomain::Int(ranges) = dom.as_ref() else {
        return Err(RuleNotApplicable);
    };
    let (min, max) = ranges
        .iter()
        .fold((i32::MAX, i32::MIN), |(min_a, max_b), range| {
            (
                min_a.min(*range.low().unwrap()),
                max_b.max(*range.high().unwrap()),
            )
        });

    let mut symbols = symbols.clone();
    let name = name.name().to_owned();
    let repr_exists = symbols.get_representation(&name, &["int_log"]).is_some();
    let representation = symbols
        .get_or_add_representation(&name, &["int_log"])
        .ok_or(RuleNotApplicable)?;
    let bits = representation[0]
        .clone()
        .expression_down(&symbols)?
        .into_values()
        .collect();
    let cnf_int = Expr::SATInt(
        Metadata::new(),
        SATIntEncoding::Log,
        Moo::new(into_matrix_expr!(bits)),
        (min, max),
    );

    if repr_exists {
        Ok(RuleEffect::pure(cnf_int))
    } else {
        Ok(RuleEffect::new(
            cnf_int.clone(),
            vec![int_domain_to_expr(cnf_int, ranges)],
            symbols,
        ))
    }
}

/// Convert an integer literal to logarithmic SAT integer form.
#[register_rule("SAT_Log", 9500, [Atomic])]
fn literal_cnf_int(expr: &Expr, _: &SymbolTable) -> ApplicationResult {
    let Expr::Atomic(_, Atom::Literal(Literal::Int(value))) = expr else {
        return Err(RuleNotApplicable);
    };

    let mut binary_encoding = vec![];
    let mut remaining = *value as u32;
    for _ in 0..bit_magnitude(*value) {
        binary_encoding.push(Expr::Atomic(
            Metadata::new(),
            Atom::Literal(Literal::Bool((remaining & 1) != 0)),
        ));
        remaining >>= 1;
    }

    Ok(RuleEffect::pure(Expr::SATInt(
        Metadata::new(),
        SATIntEncoding::Log,
        Moo::new(into_matrix_expr!(binary_encoding)),
        (*value, *value),
    )))
}

/// Number of bits required to encode an `i32` in two's complement.
pub(super) fn bit_magnitude(value: i32) -> usize {
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
