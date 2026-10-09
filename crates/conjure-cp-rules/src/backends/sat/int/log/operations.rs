use conjure_cp::ast::Expression as Expr;
use conjure_cp::ast::{SATIntEncoding, SymbolTable};
use conjure_cp::rule_engine::{
    ApplicationError::RuleNotApplicable, ApplicationResult, RuleEffect, register_rule,
};

use conjure_cp::ast::AbstractLiteral::Matrix;
use conjure_cp::ast::Metadata;
use conjure_cp::ast::Moo;
use conjure_cp::into_matrix_expr;

use itertools::Itertools;

use super::materialise::{bit_magnitude, match_bits_length, validate_log_int_operands};
use crate::backends::sat::boolean::{
    tseytin_and, tseytin_iff, tseytin_imply, tseytin_mux, tseytin_not, tseytin_or, tseytin_xor,
};

use conjure_cp::ast::SatEncodingDecision;

use std::cmp;

// Creates a boolean expression for > or >=
// a > b or a >= b
// This can also be used for < and <= by reversing the order of the inputs
// Returns result, new symbol table, new clauses
fn inequality_boolean(
    a: Vec<Expr>,
    b: Vec<Expr>,
    strict: bool,
    clauses: &mut Vec<SatEncodingDecision>,
    symbols: &mut SymbolTable,
) -> Expr {
    let mut notb;
    let mut output;

    if strict {
        notb = tseytin_not(b[0].clone(), clauses, symbols);
        output = tseytin_and(&[a[0].clone(), notb], clauses, symbols);
    } else {
        output = tseytin_imply(b[0].clone(), a[0].clone(), clauses, symbols);
    }

    //TODO: There may be room for simplification, and constant optimisation

    let bit_count = a.len();

    let mut lhs;
    let mut rhs;
    let mut iff;
    for n in 1..(bit_count - 1) {
        notb = tseytin_not(b[n].clone(), clauses, symbols);
        lhs = tseytin_and(&[a[n].clone(), notb.clone()], clauses, symbols);
        iff = tseytin_iff(a[n].clone(), b[n].clone(), clauses, symbols);
        rhs = tseytin_and(&[iff.clone(), output.clone()], clauses, symbols);
        output = tseytin_or(&[lhs.clone(), rhs.clone()], clauses, symbols);
    }

    // final bool is the sign bit and should be handled inversely
    let nota = tseytin_not(a[bit_count - 1].clone(), clauses, symbols);
    lhs = tseytin_and(&[nota, b[bit_count - 1].clone()], clauses, symbols);
    iff = tseytin_iff(
        a[bit_count - 1].clone(),
        b[bit_count - 1].clone(),
        clauses,
        symbols,
    );
    rhs = tseytin_and(&[iff, output.clone()], clauses, symbols);
    output = tseytin_or(&[lhs, rhs], clauses, symbols);

    output
}

/// Returns result, new symbol table, new clauses
/// This function expects bits to match the lengths of x and y
fn tseytin_int_adder(
    x: &[Expr],
    y: &[Expr],
    bits: usize,
    clauses: &mut Vec<SatEncodingDecision>,
    symbols: &mut SymbolTable,
) -> Vec<Expr> {
    //TODO: Optimizing for constants
    let (mut result, mut carry) = tseytin_half_adder(x[0].clone(), y[0].clone(), clauses, symbols);

    let mut output = vec![result];
    for i in 1..bits {
        (result, carry) =
            tseytin_full_adder(x[i].clone(), y[i].clone(), carry.clone(), clauses, symbols);
        output.push(result);
    }

    output
}

/// This function adds two booleans and a carry boolean using the full-adder logic circuit, it is intended for use in a binary adder.
fn tseytin_full_adder(
    a: Expr,
    b: Expr,
    carry: Expr,
    clauses: &mut Vec<SatEncodingDecision>,
    symbols: &mut SymbolTable,
) -> (Expr, Expr) {
    let axorb = tseytin_xor(a.clone(), b.clone(), clauses, symbols);
    let result = tseytin_xor(axorb.clone(), carry.clone(), clauses, symbols);
    let aandb = tseytin_and(&[a, b], clauses, symbols);
    let carryandaxorb = tseytin_and(&[carry, axorb], clauses, symbols);
    let carryout = tseytin_or(&[aandb, carryandaxorb], clauses, symbols);

    (result, carryout)
}

/// This function adds two booleans using the half-adder logic circuit, it is intended for use in a binary adder.
fn tseytin_half_adder(
    a: Expr,
    b: Expr,
    clauses: &mut Vec<SatEncodingDecision>,
    symbols: &mut SymbolTable,
) -> (Expr, Expr) {
    let result = tseytin_xor(a.clone(), b.clone(), clauses, symbols);
    let carry = tseytin_and(&[a, b], clauses, symbols);

    (result, carry)
}

/// this function is for specifically adding a power of two constant to a cnf int.
fn tseytin_add_two_power(
    expr: &[Expr],
    exponent: usize,
    bits: usize,
    clauses: &mut Vec<SatEncodingDecision>,
    symbols: &mut SymbolTable,
) -> Vec<Expr> {
    let mut result = vec![];
    let mut product = expr[exponent].clone();

    for item in expr.iter().take(exponent) {
        result.push(item.clone());
    }

    result.push(tseytin_not(expr[exponent].clone(), clauses, symbols));

    for item in expr.iter().take(bits).skip(exponent + 1) {
        result.push(tseytin_xor(product.clone(), item.clone(), clauses, symbols));
        product = tseytin_and(&[product, item.clone()], clauses, symbols);
    }

    result
}

/// This function multiplies two binary values using the shift-add multiplication algorithm.
fn cnf_shift_add_multiply(
    x: &[Expr],
    y: &[Expr],
    bits: usize,
    clauses: &mut Vec<SatEncodingDecision>,
    symbols: &mut SymbolTable,
) -> Vec<Expr> {
    let mut x = x.to_owned();
    let mut y = y.to_owned();

    //TODO Optimizing for constants
    //TODO Optimize addition for i left shifted values - skip first i bits

    // extend sign bits of operands to 2*`bits`
    x.extend(std::iter::repeat_n(x[bits - 1].clone(), bits));
    y.extend(std::iter::repeat_n(y[bits - 1].clone(), bits));

    let mut s: Vec<Expr> = vec![];
    let mut x_0andy_i;

    for bit in &y {
        x_0andy_i = tseytin_and(&[x[0].clone(), bit.clone()], clauses, symbols);
        s.push(x_0andy_i);
    }

    let mut sum;
    let mut if_true;
    let mut not_x_n;
    let mut if_false;

    // Include the extended sign bits in the full-width modular product.
    for item in x.iter().take(bits * 2).skip(1) {
        // y << 1
        for i in (1..bits * 2).rev() {
            y[i] = y[i - 1].clone();
        }
        y[0] = false.into();

        // TODO switch to multiplexer
        sum = tseytin_int_adder(&s, &y, bits * 2, clauses, symbols);
        not_x_n = tseytin_not(item.clone(), clauses, symbols);

        for i in 0..(bits * 2) {
            if_true = tseytin_and(&[item.clone(), sum[i].clone()], clauses, symbols);
            if_false = tseytin_and(&[not_x_n.clone(), s[i].clone()], clauses, symbols);
            s[i] = tseytin_or(&[if_true.clone(), if_false.clone()], clauses, symbols);
        }
    }

    s
}

/// This function calculates the range of the product of multiple integers.
/// E.g.
/// a : [2, 5], b : [-1, 2], c : [-10, -6], d : [0, 3]
/// a * b * c *d : [-300, 150]
fn product_of_ranges(ranges: Vec<&(i32, i32)>) -> (i32, i32) {
    if ranges.is_empty() {
        return (1, 1); // product of zero numbers = 1
    }

    let &(mut min_prod, mut max_prod) = ranges[0];

    for &(a, b) in &ranges[1..] {
        let candidates = [min_prod * a, min_prod * b, max_prod * a, max_prod * b];
        min_prod = *candidates.iter().min().unwrap();
        max_prod = *candidates.iter().max().unwrap();
    }

    (min_prod, max_prod)
}

/// Converts product of SATInts to a single SATInt
///
/// ```text
/// Product(SATInt(a), SATInt(b), ...) ~> SATInt(c)
///
/// ```
#[register_rule("SAT", 9000, [Product])]
fn cnf_int_product(expr: &Expr, symbols: &SymbolTable) -> ApplicationResult {
    // Constant scaling belongs to the selected linear encoder, not multiplication circuits.
    if crate::backends::sat::pseudo_boolean::integer_view(expr).is_some() {
        return Err(RuleNotApplicable);
    }
    let Expr::Product(_, exprs) = expr else {
        return Err(RuleNotApplicable);
    };

    let Expr::AbstractLiteral(_, Matrix(exprs_list, _)) = exprs.as_ref() else {
        return Err(RuleNotApplicable);
    };

    let ranges: Result<Vec<_>, _> = exprs_list
        .iter()
        .map(|e| match e {
            Expr::SATInt(_, _, _, x) => Ok(x),
            _ => Err(RuleNotApplicable),
        })
        .collect();

    let ranges = ranges?; // propagate error if any

    let (min, max) = product_of_ranges(ranges.clone());

    let exprs_bits = validate_log_int_operands(exprs_list.clone(), None)?;

    let mut new_symbols = symbols.clone();
    let mut new_sat_decisions = vec![];

    let (result, _) = exprs_bits
        .iter()
        .cloned()
        .zip(ranges.into_iter().copied())
        .reduce(|lhs, rhs| {
            // Make both bit vectors the same length
            let (lhs_bits, rhs_bits) = match_bits_length(lhs.0.clone(), rhs.0.clone());

            // Multiply operands
            let mut values = cnf_shift_add_multiply(
                &lhs_bits,
                &rhs_bits,
                lhs_bits.len(),
                &mut new_sat_decisions,
                &mut new_symbols,
            );

            // Determine new range of result
            let (mut cum_min, mut cum_max) = lhs.1;
            let candidates = [
                cum_min * rhs.1.0,
                cum_min * rhs.1.1,
                cum_max * rhs.1.0,
                cum_max * rhs.1.1,
            ];
            cum_min = *candidates.iter().min().unwrap();
            cum_max = *candidates.iter().max().unwrap();

            let new_bit_count = bit_magnitude(cum_min).max(bit_magnitude(cum_max));
            values.truncate(new_bit_count);

            (values, (cum_min, cum_max))
        })
        .unwrap();

    Ok(RuleEffect::sat(
        Expr::SATInt(
            Metadata::new(),
            SATIntEncoding::Log,
            Moo::new(into_matrix_expr!(result)),
            (min, max),
        ),
        new_sat_decisions,
        new_symbols,
    ))
}

fn tseytin_negate(
    expr: &Vec<Expr>,
    bits: usize,
    clauses: &mut Vec<SatEncodingDecision>,
    symbols: &mut SymbolTable,
) -> Vec<Expr> {
    let mut result = vec![];
    // invert bits
    for bit in expr {
        result.push(tseytin_not(bit.clone(), clauses, symbols));
    }

    // add one
    result = tseytin_add_two_power(&result, 0, bits, clauses, symbols);

    result
}

/// Converts min of SATInts to a single SATInt
///
/// ```text
/// Min(SATInt(a), SATInt(b), ...) ~> SATInt(c)
///
/// ```
#[register_rule("SAT", 4100, [Min])]
fn cnf_int_min(expr: &Expr, symbols: &SymbolTable) -> ApplicationResult {
    let Expr::Min(_, exprs) = expr else {
        return Err(RuleNotApplicable);
    };

    let Expr::AbstractLiteral(_, Matrix(exprs_list, _)) = exprs.as_ref() else {
        return Err(RuleNotApplicable);
    };

    let ranges: Result<Vec<_>, _> = exprs_list
        .iter()
        .map(|e| match e {
            Expr::SATInt(_, _, _, x) => Ok(x),
            _ => Err(RuleNotApplicable),
        })
        .collect();

    let ranges = ranges?; // propagate error if any

    // Is this optimal?
    let min = ranges.iter().map(|(a, _)| *a).min().unwrap();
    let max = ranges.iter().map(|(_, b)| *b).min().unwrap();

    let mut exprs_bits = validate_log_int_operands(exprs_list.clone(), None)?;

    let mut new_symbols = symbols.clone();
    let mut values;
    let mut new_sat_decisions = vec![];

    while exprs_bits.len() > 1 {
        let mut next = Vec::with_capacity(exprs_bits.len().div_ceil(2));
        let mut iter = exprs_bits.into_iter();

        while let Some(a) = iter.next() {
            if let Some(b) = iter.next() {
                values =
                    tseytin_binary_min_max(&a, &b, true, &mut new_sat_decisions, &mut new_symbols);
                next.push(values);
            } else {
                next.push(a);
            }
        }

        exprs_bits = next;
    }

    let result = exprs_bits.pop().unwrap();

    Ok(RuleEffect::sat(
        Expr::SATInt(
            Metadata::new(),
            SATIntEncoding::Log,
            Moo::new(into_matrix_expr!(result)),
            (min, max),
        ),
        new_sat_decisions,
        new_symbols,
    ))
}

/// General function for getting the min or max of two log integers.
fn tseytin_binary_min_max(
    x: &[Expr],
    y: &[Expr],
    min: bool,
    clauses: &mut Vec<SatEncodingDecision>,
    symbols: &mut SymbolTable,
) -> Vec<Expr> {
    let mask = if min {
        // mask is 1 if x > y
        inequality_boolean(x.to_owned(), y.to_owned(), true, clauses, symbols)
    } else {
        // flip the args if getting maximum x < y -> 1
        inequality_boolean(y.to_owned(), x.to_owned(), true, clauses, symbols)
    };

    tseytin_select_array(mask, x, y, clauses, symbols)
}

// Selects between two boolean vectors depending on a condition (both vectors must be the same length)
/// cond ? b : a
///
/// cond = 1 => b
/// cond = 0 => a
fn tseytin_select_array(
    cond: Expr,
    a: &[Expr],
    b: &[Expr],
    clauses: &mut Vec<SatEncodingDecision>,
    symbols: &mut SymbolTable,
) -> Vec<Expr> {
    assert_eq!(
        a.len(),
        b.len(),
        "Input vectors 'a' and 'b' must have the same length"
    );

    let mut out = vec![];

    let bit_count = a.len();

    for i in 0..bit_count {
        out.push(tseytin_mux(
            cond.clone(),
            a[i].clone(),
            b[i].clone(),
            clauses,
            symbols,
        ));
    }

    out
}

/// Converts max of SATInts to a single SATInt
///
/// ```text
/// Max(SATInt(a), SATInt(b), ...) ~> SATInt(c)
///
/// ```
#[register_rule("SAT", 4100, [Max])]
fn cnf_int_max(expr: &Expr, symbols: &SymbolTable) -> ApplicationResult {
    let Expr::Max(_, exprs) = expr else {
        return Err(RuleNotApplicable);
    };

    let Expr::AbstractLiteral(_, Matrix(exprs_list, _)) = exprs.as_ref() else {
        return Err(RuleNotApplicable);
    };

    let ranges: Result<Vec<_>, _> = exprs_list
        .iter()
        .map(|e| match e {
            Expr::SATInt(_, _, _, x) => Ok(x),
            _ => Err(RuleNotApplicable),
        })
        .collect();

    let ranges = ranges?; // propagate error if any

    // Is this optimal?
    let min = ranges.iter().map(|(a, _)| *a).max().unwrap();
    let max = ranges.iter().map(|(_, b)| *b).max().unwrap();

    let mut exprs_bits = validate_log_int_operands(exprs_list.clone(), None)?;

    let mut new_symbols = symbols.clone();
    let mut values;
    let mut new_sat_decisions = vec![];

    while exprs_bits.len() > 1 {
        let mut next = Vec::with_capacity(exprs_bits.len().div_ceil(2));
        let mut iter = exprs_bits.into_iter();

        while let Some(a) = iter.next() {
            if let Some(b) = iter.next() {
                values =
                    tseytin_binary_min_max(&a, &b, false, &mut new_sat_decisions, &mut new_symbols);
                next.push(values);
            } else {
                next.push(a);
            }
        }

        exprs_bits = next;
    }

    let result = exprs_bits.pop().unwrap();

    Ok(RuleEffect::sat(
        Expr::SATInt(
            Metadata::new(),
            SATIntEncoding::Log,
            Moo::new(into_matrix_expr!(result)),
            (min, max),
        ),
        new_sat_decisions,
        new_symbols,
    ))
}

/// Converts Abs of a SATInt to a SATInt
///
/// ```text
/// |SATInt(a)| ~> SATInt(b)
///
/// ```
#[register_rule("SAT", 4100, [Abs])]
fn cnf_int_abs(expr: &Expr, symbols: &SymbolTable) -> ApplicationResult {
    let Expr::Abs(_, expr) = expr else {
        return Err(RuleNotApplicable);
    };

    let Expr::SATInt(_, _, _, (min, max)) = expr.as_ref() else {
        return Err(RuleNotApplicable);
    };

    let range = (
        cmp::max(0, cmp::max(*min, -*max)),
        cmp::max(min.abs(), max.abs()),
    );

    let binding = validate_log_int_operands(vec![expr.as_ref().clone()], None)?;
    let [bits] = binding.as_slice() else {
        return Err(RuleNotApplicable);
    };

    let mut new_sat_decisions = vec![];
    let mut new_symbols = symbols.clone();

    let mut result = vec![];

    // How does this handle negatives edge cases: -(-8) = 8, an extra bit is needed

    // invert bits
    for bit in bits {
        result.push(tseytin_not(
            bit.clone(),
            &mut new_sat_decisions,
            &mut new_symbols,
        ));
    }

    let bit_count = result.len();

    // add one
    result = tseytin_add_two_power(
        &result,
        0,
        bit_count,
        &mut new_sat_decisions,
        &mut new_symbols,
    );

    for i in 0..bit_count {
        result[i] = tseytin_mux(
            bits[bit_count - 1].clone(),
            bits[i].clone(),
            result[i].clone(),
            &mut new_sat_decisions,
            &mut new_symbols,
        )
    }

    Ok(RuleEffect::sat(
        Expr::SATInt(
            Metadata::new(),
            SATIntEncoding::Log,
            Moo::new(into_matrix_expr!(result)),
            range,
        ),
        new_sat_decisions,
        new_symbols,
    ))
}

/// Converts SafeDiv of SATInts to a single SATInt
///
/// ```text
/// SafeDiv(SATInt(a), SATInt(b)) ~> SATInt(c)
///
/// ```
#[register_rule("SAT", 4100, [SafeDiv])]
fn cnf_int_safediv(expr: &Expr, symbols: &SymbolTable) -> ApplicationResult {
    let Expr::SafeDiv(_, numer, denom) = expr else {
        return Err(RuleNotApplicable);
    };
    encode_div_mod(numer, denom, symbols, false)
}

/// Reuses restoring division's remainder, with Essence's floor-modulo semantics.
#[register_rule("SAT", 4100, [SafeMod])]
fn cnf_int_safemod(expr: &Expr, symbols: &SymbolTable) -> ApplicationResult {
    let Expr::SafeMod(_, numer, denom) = expr else {
        return Err(RuleNotApplicable);
    };
    encode_div_mod(numer, denom, symbols, true)
}

fn encode_div_mod(
    numer: &Expr,
    denom: &Expr,
    symbols: &SymbolTable,
    modulo: bool,
) -> ApplicationResult {
    // Restoring division computes both quotient and remainder magnitudes.

    let Expr::SATInt(_, _, _, (numer_min, numer_max)) = numer else {
        return Err(RuleNotApplicable);
    };

    let Expr::SATInt(_, _, _, (denom_min, denom_max)) = denom else {
        return Err(RuleNotApplicable);
    };

    let (min, max) = if modulo {
        // Remainders have the divisor's sign; include the safe zero-divisor dummy.
        (
            if *denom_min < 0 { denom_min + 1 } else { 0 },
            if *denom_max > 0 { denom_max - 1 } else { 0 },
        )
    } else {
        division_bounds((*numer_min, *numer_max), (*denom_min, *denom_max))
            .ok_or(RuleNotApplicable)?
    };

    let binding = validate_log_int_operands(vec![numer.clone(), denom.clone()], None)?;
    let [numer_bits, denom_bits] = binding.as_slice() else {
        return Err(RuleNotApplicable);
    };

    // Widen before taking absolute values: abs(MIN) needs an additional sign bit.
    let mut numer_bits = numer_bits.clone();
    let mut denom_bits = denom_bits.clone();
    numer_bits.push(numer_bits.last().unwrap().clone());
    denom_bits.push(denom_bits.last().unwrap().clone());
    let bit_count = numer_bits.len();

    let mut new_symbols = symbols.clone();
    let mut new_sat_decisions = vec![];
    let mut quotient = vec![false.into(); bit_count];

    let minus_numer = tseytin_negate(
        &numer_bits.clone(),
        bit_count,
        &mut new_sat_decisions,
        &mut new_symbols,
    );
    let minus_denom = tseytin_negate(
        &denom_bits.clone(),
        bit_count,
        &mut new_sat_decisions,
        &mut new_symbols,
    );

    let denom_sign = denom_bits[bit_count - 1].clone();
    let sign_bit = tseytin_xor(
        numer_bits[bit_count - 1].clone(),
        denom_bits[bit_count - 1].clone(),
        &mut new_sat_decisions,
        &mut new_symbols,
    );

    let numer_bits = tseytin_select_array(
        numer_bits[bit_count - 1].clone(),
        &numer_bits.clone(),
        &minus_numer,
        &mut new_sat_decisions,
        &mut new_symbols,
    );
    let denom_bits = tseytin_select_array(
        denom_bits[bit_count - 1].clone(),
        &denom_bits.clone(),
        &minus_denom,
        &mut new_sat_decisions,
        &mut new_symbols,
    );

    let mut r = numer_bits;
    r.extend(std::iter::repeat_n(r[bit_count - 1].clone(), bit_count));
    let mut d = std::iter::repeat_n(false.into(), bit_count).collect_vec();
    d.extend(denom_bits.clone());

    let minus_d = tseytin_negate(
        &d.clone(),
        2 * bit_count,
        &mut new_sat_decisions,
        &mut new_symbols,
    );
    let mut rminusd;

    for i in (0..bit_count).rev() {
        // r << 1
        for j in (1..bit_count * 2).rev() {
            r[j] = r[j - 1].clone();
        }
        r[0] = false.into();

        rminusd = tseytin_int_adder(
            &r.clone(),
            &minus_d.clone(),
            2 * bit_count,
            &mut new_sat_decisions,
            &mut new_symbols,
        );

        quotient[i] = tseytin_not(
            // q[i] = inverse of sign bit - 1 if positive, 0 if negative
            rminusd[2 * bit_count - 1].clone(),
            &mut new_sat_decisions,
            &mut new_symbols,
        );

        for j in 0..(2 * bit_count) {
            r[j] = tseytin_mux(
                quotient[i].clone(),
                r[j].clone(),       // use r if negative
                rminusd[j].clone(), // use r-d if positive
                &mut new_sat_decisions,
                &mut new_symbols,
            );
        }
    }

    // Opposite signs and a nonzero remainder require one more unit of magnitude
    // before negation to obtain floor division rather than truncating division.
    let remainder_nonzero = tseytin_or(&r, &mut new_sat_decisions, &mut new_symbols);
    let round_down = tseytin_and(
        &[sign_bit.clone(), remainder_nonzero],
        &mut new_sat_decisions,
        &mut new_symbols,
    );
    let out = if modulo {
        let remainder = r[bit_count..].to_vec();
        let minus_remainder = tseytin_negate(
            &remainder,
            bit_count,
            &mut new_sat_decisions,
            &mut new_symbols,
        );
        let complement = tseytin_int_adder(
            &denom_bits,
            &minus_remainder,
            bit_count,
            &mut new_sat_decisions,
            &mut new_symbols,
        );
        let magnitude = tseytin_select_array(
            round_down,
            &remainder,
            &complement,
            &mut new_sat_decisions,
            &mut new_symbols,
        );
        let negative = tseytin_negate(
            &magnitude,
            bit_count,
            &mut new_sat_decisions,
            &mut new_symbols,
        );
        let signed = tseytin_select_array(
            denom_sign,
            &magnitude,
            &negative,
            &mut new_sat_decisions,
            &mut new_symbols,
        );
        // A safe zero divisor must not restrict the operands or escape the result bounds.
        let nonzero = tseytin_or(&denom_bits, &mut new_sat_decisions, &mut new_symbols);
        tseytin_select_array(
            nonzero,
            &vec![false.into(); bit_count],
            &signed,
            &mut new_sat_decisions,
            &mut new_symbols,
        )
    } else {
        let mut correction = vec![false.into(); bit_count];
        correction[0] = round_down;
        quotient = tseytin_int_adder(
            &quotient,
            &correction,
            bit_count,
            &mut new_sat_decisions,
            &mut new_symbols,
        );

        let minus_quotient = tseytin_negate(
            &quotient.clone(),
            bit_count,
            &mut new_sat_decisions,
            &mut new_symbols,
        );

        tseytin_select_array(
            sign_bit,
            &quotient,
            &minus_quotient,
            &mut new_sat_decisions,
            &mut new_symbols,
        )
    };

    Ok(RuleEffect::sat(
        Expr::SATInt(
            Metadata::new(),
            SATIntEncoding::Log,
            Moo::new(into_matrix_expr!(out)),
            (min, max),
        ),
        new_sat_decisions,
        new_symbols,
    ))
}

// Quotient extrema can occur at the nonzero denominators closest to zero.
fn division_bounds(numerator: (i32, i32), denominator: (i32, i32)) -> Option<(i32, i32)> {
    let mut values = Vec::new();
    for divisor in [denominator.0, denominator.1, -1, 1] {
        if divisor == 0 || divisor < denominator.0 || divisor > denominator.1 {
            continue;
        }
        for dividend in [numerator.0, numerator.1] {
            values.push(super::super::floor_div(dividend, divisor)?);
        }
    }
    if denominator.0 <= 0 && denominator.1 >= 0 {
        values.push(0);
    }
    Some((*values.iter().min()?, *values.iter().max()?))
}

/// Lower power using exponentiation by squaring and the existing Boolean circuit decisions.
/// Undefined inputs retain a harmless value; their definedness is handled by the bubble rules.
#[register_rule("SAT", 4100, [SafePow])]
fn cnf_int_safepow(expr: &Expr, symbols: &SymbolTable) -> ApplicationResult {
    let Expr::SafePow(_, base, exponent) = expr else {
        return Err(RuleNotApplicable);
    };
    let (
        Expr::SATInt(_, SATIntEncoding::Log, _, base_bounds),
        Expr::SATInt(_, SATIntEncoding::Log, exponent_bits, exponent_bounds),
    ) = (base.as_ref(), exponent.as_ref())
    else {
        return Err(RuleNotApplicable);
    };
    // Endpoints bound both parities; include zero and the unspecified values 0/1.
    // Decline machine-overflowing ranges instead of silently wrapping their mathematical powers.
    let mut bounds = (0, 1);
    if exponent_bounds.1 > 31 && (base_bounds.0 < -1 || base_bounds.1 > 1) {
        return Err(RuleNotApplicable);
    }
    for power in 0..=exponent_bounds.1.clamp(0, 31) {
        for value in [base_bounds.0, base_bounds.1] {
            let result = value.checked_pow(power as u32).ok_or(RuleNotApplicable)?;
            bounds.0 = bounds.0.min(result);
            bounds.1 = bounds.1.max(result);
        }
    }
    let width = bit_magnitude(bounds.0).max(bit_magnitude(bounds.1)).max(2);
    let mut factor =
        validate_log_int_operands(vec![(**base).clone()], Some(width as u32))?.remove(0);
    let exponent_bits = exponent_bits.unwrap_list_cow().ok_or(RuleNotApplicable)?;
    let mut result = vec![Expr::from(false); width];
    result[0] = true.into();
    let mut decisions = vec![];
    let mut symbols = symbols.clone();
    let mut result_is_one = true;
    // Only non-sign bits can participate in a defined, non-negative exponent.
    for (index, selected) in exponent_bits[..exponent_bits.len() - 1].iter().enumerate() {
        let selected_constant = conjure_cp::ast::eval_constant(selected);
        if selected_constant != Some(false.into()) {
            let mut product = if result_is_one {
                factor.clone()
            } else {
                cnf_shift_add_multiply(&result, &factor, width, &mut decisions, &mut symbols)
            };
            product.truncate(width);
            result = if selected_constant == Some(true.into()) {
                product
            } else {
                result
                    .iter()
                    .zip(product)
                    .map(|(old, new)| {
                        tseytin_mux(
                            selected.clone(),
                            old.clone(),
                            new,
                            &mut decisions,
                            &mut symbols,
                        )
                    })
                    .collect()
            };
            result_is_one = false;
        }
        if index + 1 < exponent_bits.len() - 1 {
            factor = cnf_shift_add_multiply(&factor, &factor, width, &mut decisions, &mut symbols);
            factor.truncate(width);
        }
    }
    let negative = exponent_bits.last().ok_or(RuleNotApplicable)?;
    if conjure_cp::ast::eval_constant(negative) != Some(false.into()) {
        for bit in &mut result {
            *bit = tseytin_mux(
                negative.clone(),
                bit.clone(),
                false.into(),
                &mut decisions,
                &mut symbols,
            );
        }
    }
    Ok(RuleEffect::sat(
        Expr::SATInt(
            Metadata::new(),
            SATIntEncoding::Log,
            Moo::new(into_matrix_expr!(result)),
            bounds,
        ),
        decisions,
        symbols,
    ))
}
