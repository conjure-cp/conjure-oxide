use conjure_cp::ast::Expression as Expr;
use conjure_cp::ast::{SATIntEncoding, SymbolTable};
use conjure_cp::rule_engine::{
    ApplicationError::RuleNotApplicable, ApplicationResult, Reduction, register_rule,
};

use conjure_cp::ast::AbstractLiteral::Matrix;
use conjure_cp::ast::Metadata;
use conjure_cp::ast::Moo;
use conjure_cp::into_matrix_expr;

use itertools::Itertools;

use crate::sat::integer_repr::int_to_log;

use super::boolean::{
    tseytin_and, tseytin_iff, tseytin_imply, tseytin_mux, tseytin_not, tseytin_or, tseytin_xor,
};
use super::integer_repr::{bit_magnitude, match_bits_length, validate_log_int_operands};

use conjure_cp::ast::CnfClause;

use std::cmp;
use std::ops::Div;

/// Converts an inequality expression between two SATInts to a boolean expression in cnf.
///
/// ```text
/// SATInt(a) </>/<=/>= SATInt(b) ~> Bool
///
/// ```
#[register_rule(("SAT_Log", 4100))]
fn cnf_int_ineq(expr: &Expr, symbols: &SymbolTable) -> ApplicationResult {
    let (lhs, rhs, strict) = match expr {
        Expr::Lt(_, x, y) => (y, x, true),
        Expr::Gt(_, x, y) => (x, y, true),
        Expr::Leq(_, x, y) => (y, x, false),
        Expr::Geq(_, x, y) => (x, y, false),
        _ => return Err(RuleNotApplicable),
    };

    let binding =
        validate_log_int_operands(vec![lhs.as_ref().clone(), rhs.as_ref().clone()], None)?;
    let [lhs_bits, rhs_bits] = binding.as_slice() else {
        return Err(RuleNotApplicable);
    };

    let mut new_symbols = symbols.clone();
    let mut new_clauses = vec![];

    let output = inequality_boolean(
        lhs_bits.clone(),
        rhs_bits.clone(),
        strict,
        &mut new_clauses,
        &mut new_symbols,
    );
    Ok(Reduction::cnf(output, new_clauses, new_symbols))
}

/// Converts a = expression between two SATInts to a boolean expression in cnf
///
/// ```text
/// SATInt(a) = SATInt(b) ~> Bool
///
/// ```
#[register_rule(("SAT_Log", 9100))]
fn cnf_int_eq(expr: &Expr, symbols: &SymbolTable) -> ApplicationResult {
    let Expr::Eq(_, lhs, rhs) = expr else {
        return Err(RuleNotApplicable);
    };

    let binding =
        validate_log_int_operands(vec![lhs.as_ref().clone(), rhs.as_ref().clone()], None)?;
    let [lhs_bits, rhs_bits] = binding.as_slice() else {
        return Err(RuleNotApplicable);
    };

    let bit_count = lhs_bits.len();

    let mut output = true.into();
    let mut new_symbols = symbols.clone();
    let mut new_clauses = vec![];
    let mut comparison;

    for i in 0..bit_count {
        comparison = tseytin_iff(
            lhs_bits[i].clone(),
            rhs_bits[i].clone(),
            &mut new_clauses,
            &mut new_symbols,
        );
        output = tseytin_and(
            &vec![comparison, output],
            &mut new_clauses,
            &mut new_symbols,
        );
    }

    Ok(Reduction::cnf(output, new_clauses, new_symbols))
}

/// Converts a != expression between two SATInts to a boolean expression in cnf
///
/// ```text
/// SATInt(a) != SATInt(b) ~> Bool
///
/// ```
#[register_rule(("SAT_Log", 4100))]
fn cnf_int_neq(expr: &Expr, symbols: &SymbolTable) -> ApplicationResult {
    let Expr::Neq(_, lhs, rhs) = expr else {
        return Err(RuleNotApplicable);
    };

    validate_log_int_operands(vec![lhs.as_ref().clone(), rhs.as_ref().clone()], None)?;

    let mut new_symbols = symbols.clone();
    let mut new_clauses = vec![];

    let result = log_neq(lhs.as_ref(), rhs.as_ref(), &mut new_clauses, &mut new_symbols);

    Ok(Reduction::cnf(result, new_clauses, new_symbols))
}

// Creates a boolean expression for > or >=
// a > b or a >= b
// This can also be used for < and <= by reversing the order of the inputs
// Returns result, new symbol table, new clauses
fn inequality_boolean(
    a: Vec<Expr>,
    b: Vec<Expr>,
    strict: bool,
    clauses: &mut Vec<CnfClause>,
    symbols: &mut SymbolTable,
) -> Expr {
    let mut notb;
    let mut output;

    if strict {
        notb = tseytin_not(b[0].clone(), clauses, symbols);
        output = tseytin_and(&vec![a[0].clone(), notb], clauses, symbols);
    } else {
        output = tseytin_imply(b[0].clone(), a[0].clone(), clauses, symbols);
    }

    //TODO: There may be room for simplification, and constant optimization

    let bit_count = a.len();

    let mut lhs;
    let mut rhs;
    let mut iff;
    for n in 1..(bit_count - 1) {
        notb = tseytin_not(b[n].clone(), clauses, symbols);
        lhs = tseytin_and(&vec![a[n].clone(), notb.clone()], clauses, symbols);
        iff = tseytin_iff(a[n].clone(), b[n].clone(), clauses, symbols);
        rhs = tseytin_and(&vec![iff.clone(), output.clone()], clauses, symbols);
        output = tseytin_or(&vec![lhs.clone(), rhs.clone()], clauses, symbols);
    }

    // final bool is the sign bit and should be handled inversely
    let nota = tseytin_not(a[bit_count - 1].clone(), clauses, symbols);
    lhs = tseytin_and(&vec![nota, b[bit_count - 1].clone()], clauses, symbols);
    iff = tseytin_iff(
        a[bit_count - 1].clone(),
        b[bit_count - 1].clone(),
        clauses,
        symbols,
    );
    rhs = tseytin_and(&vec![iff, output.clone()], clauses, symbols);
    output = tseytin_or(&vec![lhs, rhs], clauses, symbols);

    output
}

/// Converts sum of SATInts to a single SATInt
///
/// ```text
/// Sum(SATInt(a), SATInt(b), ...) ~> SATInt(c)
///
/// ```
#[register_rule(("SAT_Log", 4100))]
fn cnf_int_sum(expr: &Expr, symbols: &SymbolTable) -> ApplicationResult {
    let Expr::Sum(_, exprs) = expr else {
        return Err(RuleNotApplicable);
    };

    let Expr::AbstractLiteral(_, Matrix(exprs_list, _)) = exprs.as_ref() else {
        return Err(RuleNotApplicable);
    };

    validate_log_int_operands(exprs_list.clone(), None)?;

    let mut new_symbols = symbols.clone();
    let mut new_clauses = vec![];

    let result = exprs_list
        .clone()
        .into_iter()
        .reduce(|lhs, rhs| log_add(&lhs, &rhs, &mut new_clauses, &mut new_symbols))
        .unwrap();

    Ok(Reduction::cnf(result, new_clauses, new_symbols))
}

/// Returns result, new symbol table, new clauses
/// This function expects bits to match the lengths of x and y
fn tseytin_int_adder(
    x: &[Expr],
    y: &[Expr],
    bits: usize,
    clauses: &mut Vec<CnfClause>,
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
    clauses: &mut Vec<CnfClause>,
    symbols: &mut SymbolTable,
) -> (Expr, Expr) {
    let axorb = tseytin_xor(a.clone(), b.clone(), clauses, symbols);
    let result = tseytin_xor(axorb.clone(), carry.clone(), clauses, symbols);
    let aandb = tseytin_and(&vec![a, b], clauses, symbols);
    let carryandaxorb = tseytin_and(&vec![carry, axorb], clauses, symbols);
    let carryout = tseytin_or(&vec![aandb, carryandaxorb], clauses, symbols);

    (result, carryout)
}

/// This function adds two booleans using the half-adder logic circuit, it is intended for use in a binary adder.
fn tseytin_half_adder(
    a: Expr,
    b: Expr,
    clauses: &mut Vec<CnfClause>,
    symbols: &mut SymbolTable,
) -> (Expr, Expr) {
    let result = tseytin_xor(a.clone(), b.clone(), clauses, symbols);
    let carry = tseytin_and(&vec![a, b], clauses, symbols);

    (result, carry)
}

/// this function is for specifically adding a power of two constant to a cnf int.
fn tseytin_add_two_power(
    expr: &[Expr],
    exponent: usize,
    bits: usize,
    clauses: &mut Vec<CnfClause>,
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
        product = tseytin_and(&vec![product, item.clone()], clauses, symbols);
    }

    result
}

pub fn log_left_shift(x: &Expr, shift: usize) -> Expr {
    let Expr::SATInt(_, SATIntEncoding::Log, moo_x_bits, (xmin, xmax)) = x else {
        panic!("Log int should always be used here");
    };

    let factor = 1 << shift;
    let min = factor * xmin;
    let max = factor * xmax;
    let bits = bit_magnitude(min).max(bit_magnitude(max));

    let mut result = moo_x_bits.as_ref().clone().unwrap_list().unwrap();
    result.splice(0..0, std::iter::repeat_n(false.into(), shift));

    result.truncate(bits);

    Expr::SATInt(
        Metadata::new(),
        SATIntEncoding::Log,
        Moo::new(into_matrix_expr!(result)),
        (min, max),
    )
}
pub fn log_negate(x: &Expr, clauses: &mut Vec<CnfClause>, symbols: &mut SymbolTable) -> Expr {
    let Expr::SATInt(_, SATIntEncoding::Log, moo_x_bits, (xmin, xmax)) = x else {
        panic!("Log int should always be used here");
    };

    let min = -xmax;
    let max = -xmin;

    let x_bits = moo_x_bits.as_ref().clone().unwrap_list().unwrap();

    let mut inv_x = vec![];
    // invert bits
    for bit in x_bits {
        inv_x.push(tseytin_not(bit.clone(), clauses, symbols));
    }

    let mut result = vec![tseytin_not(inv_x[0].clone(), clauses, symbols)];

    // add one
    let mut product = inv_x[0].clone();
    for item in inv_x.iter().skip(1) {
        result.push(tseytin_xor(product.clone(), item.clone(), clauses, symbols));
        product = tseytin_and(&vec![product, item.clone()], clauses, symbols);
    }
    result.push(tseytin_and(
        &vec![
            inv_x.last().unwrap().clone(),
            tseytin_not(product, clauses, symbols),
        ],
        clauses,
        symbols,
    ));

    let out = Expr::SATInt(
        Metadata::new(),
        SATIntEncoding::Log,
        Moo::new(into_matrix_expr!(result)),
        (min, max),
    );

    log_minimize_bits(out)
}

pub fn log_abs_sign(
    x: &Expr,
    clauses: &mut Vec<CnfClause>,
    symbols: &mut SymbolTable,
) -> (Expr, Expr) {
    let Expr::SATInt(_, SATIntEncoding::Log, moo_x_bits, (xmin, xmax)) = x else {
        panic!("Log int should always be used here");
    };

    let mut min = xmin.abs().min(xmax.abs());
    if *xmin < 0 && 0 < *xmax {
        min = 0;
    }
    let max = xmin.abs().max(xmax.abs());

    let x_bits = moo_x_bits.as_ref().clone().unwrap_list().unwrap();
    let sign = x_bits.last().unwrap().clone();

    let neg_x = log_negate(x, clauses, symbols);
    let abs_x = satint_set_range(log_select(&sign, x, &neg_x, clauses, symbols), min, max);

    (log_minimize_bits(abs_x), sign)
}

/// Multiply two log encoded integers, allowing differing ranges/bit lengths. This determines the new range and accounts for overflow
pub fn log_multiply(
    x: &Expr,
    y: &Expr,
    clauses: &mut Vec<CnfClause>,
    symbols: &mut SymbolTable,
) -> Expr {
    let Expr::SATInt(_, SATIntEncoding::Log, _, (xmin, xmax)) = x else {
        panic!("Log int should always be used here");
    };

    let Expr::SATInt(_, SATIntEncoding::Log, _, (ymin, ymax)) = y else {
        panic!("Log int should always be used here");
    };
    let binding = [xmin * ymin, xmin * ymax, xmax * ymin, xmax * ymax];
    let candidates = binding.iter();
    let min = *candidates.clone().min().unwrap();
    let max = *candidates.max().unwrap();

    let (abs_x, sign_x) = log_abs_sign(x, clauses, symbols);
    let Expr::SATInt(_, SATIntEncoding::Log, moo_abs_x_bits, _) = abs_x else {
        panic!("This should never be reached");
    };
    let abs_x_bits = moo_abs_x_bits.as_ref().clone().unwrap_list().unwrap();

    let mut shifted_y = y.clone();
    let mut result = log_select(&abs_x_bits[0], &int_to_log(0), &shifted_y, clauses, symbols);

    for x_bit in abs_x_bits.iter().skip(1) {
        shifted_y = log_left_shift(&shifted_y, 1);
        result = log_add(
            &result,
            &log_select(x_bit, &int_to_log(0), &shifted_y, clauses, symbols),
            clauses,
            symbols,
        );
    }
    result = log_select(
        &sign_x,
        &result,
        &log_negate(&result, clauses, symbols),
        clauses,
        symbols,
    );

    result = log_minimize_bits(satint_set_range(result, min, max));

    result
}

/// Add two log encoded integers, allowing differing ranges/bit lengths. This determines the new range and accounts for overflow
pub fn log_add(
    x: &Expr,
    y: &Expr,
    clauses: &mut Vec<CnfClause>,
    symbols: &mut SymbolTable,
) -> Expr {
    let Expr::SATInt(_, SATIntEncoding::Log, moo_x_bits, (xmin, xmax)) = x else {
        panic!("Log int should always be used here");
    };

    let Expr::SATInt(_, SATIntEncoding::Log, moo_y_bits, (ymin, ymax)) = y else {
        panic!("Log int should always be used here");
    };

    let min = xmin + ymin;
    let max = xmax + ymax;
    let bits = bit_magnitude(min).max(bit_magnitude(max));

    let (mut x_bits, mut y_bits) = match_bits_length(
        moo_x_bits.as_ref().clone().unwrap_list().unwrap(),
        moo_y_bits.as_ref().clone().unwrap_list().unwrap(),
    );

    // extend the sign bit to prevent overflow
    x_bits.push(x_bits.last().unwrap().clone());
    y_bits.push(y_bits.last().unwrap().clone());

    let (mut result, mut carry) =
        tseytin_half_adder(x_bits[0].clone(), y_bits[0].clone(), clauses, symbols);

    let mut output = vec![result];
    for i in 1..x_bits.len() {
        (result, carry) = tseytin_full_adder(
            x_bits[i].clone(),
            y_bits[i].clone(),
            carry.clone(),
            clauses,
            symbols,
        );
        output.push(result);
    }

    output.truncate(bits);

    Expr::SATInt(
        Metadata::new(),
        SATIntEncoding::Log,
        Moo::new(into_matrix_expr!(output)),
        (min, max),
    )
}

pub fn log_square(x: &Expr, clauses: &mut Vec<CnfClause>, symbols: &mut SymbolTable) -> Expr {
    let Expr::SATInt(_, SATIntEncoding::Log, _, (xmin, xmax)) = x else {
        panic!("Log int should always be used here");
    };
    eprintln!("ls: {}", x);

    let binding = [xmin * xmin, xmax * xmax];
    let candidates = binding.iter();
    let min = if xmin * xmax < 0 {
        0
    } else {
        *candidates.clone().min().unwrap()
    };
    let max = *candidates.max().unwrap();

    log_minimize_bits(satint_set_range(
        log_multiply(x, x, clauses, symbols),
        min,
        max,
    ))
}

pub fn log_select(
    cond: &Expr,
    x: &Expr,
    y: &Expr,
    clauses: &mut Vec<CnfClause>,
    symbols: &mut SymbolTable,
) -> Expr {
    let Expr::SATInt(_, SATIntEncoding::Log, x_bits, (xmin, xmax)) = x else {
        panic!("Log int should always be used here");
    };

    let Expr::SATInt(_, SATIntEncoding::Log, y_bits, (ymin, ymax)) = y else {
        panic!("Log int should always be used here");
    };

    let min = *xmin.min(ymin);
    let max = *xmax.max(ymax);
    let bits = bit_magnitude(min).max(bit_magnitude(max));

    let (x_match, y_match) = match_bits_length(
        x_bits.as_ref().clone().unwrap_list().unwrap(),
        y_bits.as_ref().clone().unwrap_list().unwrap(),
    );

    let mut result = tseytin_select_array(cond.clone(), &x_match, &y_match, clauses, symbols);

    result.truncate(bits);

    Expr::SATInt(
        Metadata::new(),
        SATIntEncoding::Log,
        Moo::new(into_matrix_expr!(result)),
        (min, max),
    )
}

pub fn satint_set_range(int: Expr, min: i32, max: i32) -> Expr {
    let Expr::SATInt(meta, enc, bits, (_, _)) = int else {
        panic!("Input should always be SatINT");
    };

    Expr::SATInt(meta, enc, bits, (min, max))
}

pub fn log_minimize_bits(int: Expr) -> Expr {
    let Expr::SATInt(meta, enc, bits, (min, max)) = int else {
        panic!("Input should always be SatINT");
    };

    bits.as_ref()
        .clone()
        .unwrap_list()
        .unwrap()
        .truncate(bit_magnitude(min).max(bit_magnitude(max)));

    Expr::SATInt(meta, enc, bits, (min, max))
}

/// Converts product of SATInts to a single SATInt
///
/// ```text
/// Product(SATInt(a), SATInt(b), ...) ~> SATInt(c)
///
/// ```
#[register_rule(("SAT_Log", 9000))]
fn cnf_int_product(expr: &Expr, symbols: &SymbolTable) -> ApplicationResult {
    let Expr::Product(_, exprs) = expr else {
        return Err(RuleNotApplicable);
    };

    let Expr::AbstractLiteral(_, Matrix(exprs_list, _)) = exprs.as_ref() else {
        return Err(RuleNotApplicable);
    };

    validate_log_int_operands(exprs_list.clone(), None)?;

    let mut new_symbols = symbols.clone();
    let mut new_clauses = vec![];

    let result = exprs_list
        .clone()
        .into_iter()
        .reduce(|lhs, rhs| log_multiply(&lhs, &rhs, &mut new_clauses, &mut new_symbols))
        .unwrap();

    Ok(Reduction::cnf(result, new_clauses, new_symbols))
}

/// Converts negation of a SATInt to a SATInt
///
/// ```text
/// -SATInt(a) ~> SATInt(b)
///
/// ```
#[register_rule(("SAT_Log", 4100))]
fn cnf_int_neg(expr: &Expr, symbols: &SymbolTable) -> ApplicationResult {
    let Expr::Neg(_, expr) = expr else {
        return Err(RuleNotApplicable);
    };

    validate_log_int_operands(vec![expr.as_ref().clone()], None)?;

    let mut new_clauses = vec![];
    let mut new_symbols = symbols.clone();

    let result = log_negate(expr.as_ref(), &mut new_clauses, &mut new_symbols);

    Ok(Reduction::cnf(result, new_clauses, new_symbols))
}

fn tseytin_negate(
    expr: &Vec<Expr>,
    bits: usize,
    clauses: &mut Vec<CnfClause>,
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
#[register_rule(("SAT_Log", 4100))]
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
    let mut new_clauses = vec![];

    while exprs_bits.len() > 1 {
        let mut next = Vec::with_capacity(exprs_bits.len().div_ceil(2));
        let mut iter = exprs_bits.into_iter();

        while let Some(a) = iter.next() {
            if let Some(b) = iter.next() {
                values = tseytin_binary_min_max(&a, &b, true, &mut new_clauses, &mut new_symbols);
                next.push(values);
            } else {
                next.push(a);
            }
        }

        exprs_bits = next;
    }

    let result = exprs_bits.pop().unwrap();

    Ok(Reduction::cnf(
        Expr::SATInt(
            Metadata::new(),
            SATIntEncoding::Log,
            Moo::new(into_matrix_expr!(result)),
            (min, max),
        ),
        new_clauses,
        new_symbols,
    ))
}

/// General function for getting the min or max of two log integers.
fn tseytin_binary_min_max(
    x: &[Expr],
    y: &[Expr],
    min: bool,
    clauses: &mut Vec<CnfClause>,
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
    clauses: &mut Vec<CnfClause>,
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
#[register_rule(("SAT_Log", 4100))]
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
    let mut new_clauses = vec![];

    while exprs_bits.len() > 1 {
        let mut next = Vec::with_capacity(exprs_bits.len().div_ceil(2));
        let mut iter = exprs_bits.into_iter();

        while let Some(a) = iter.next() {
            if let Some(b) = iter.next() {
                values = tseytin_binary_min_max(&a, &b, false, &mut new_clauses, &mut new_symbols);
                next.push(values);
            } else {
                next.push(a);
            }
        }

        exprs_bits = next;
    }

    let result = exprs_bits.pop().unwrap();

    Ok(Reduction::cnf(
        Expr::SATInt(
            Metadata::new(),
            SATIntEncoding::Log,
            Moo::new(into_matrix_expr!(result)),
            (min, max),
        ),
        new_clauses,
        new_symbols,
    ))
}

/// Converts Abs of a SATInt to a SATInt
///
/// ```text
/// |SATInt(a)| ~> SATInt(b)
///
/// ```
#[register_rule(("SAT_Log", 4700))]
fn cnf_int_abs(expr: &Expr, symbols: &SymbolTable) -> ApplicationResult {
    let Expr::Abs(_, expr) = expr else {
        return Err(RuleNotApplicable);
    };

    validate_log_int_operands(vec![expr.as_ref().clone()], None)?;

    let mut new_clauses = vec![];
    let mut new_symbols = symbols.clone();

    Ok(Reduction::cnf(
        log_abs_sign(expr.as_ref(), &mut new_clauses, &mut new_symbols).0,
        new_clauses,
        new_symbols,
    ))
}

/// Converts SafeDiv of SATInts to a single SATInt
///
/// ```text
/// SafeDiv(SATInt(a), SATInt(b)) ~> SATInt(c)
///
/// ```
#[register_rule(("SAT_Log", 4700))]
fn cnf_int_safediv(expr: &Expr, symbols: &SymbolTable) -> ApplicationResult {
    let Expr::SafeDiv(_, numer, denom) = expr else {
        return Err(RuleNotApplicable);
    };

    let Expr::SATInt(_, _, _, (numer_min, numer_max)) = numer.as_ref() else {
        return Err(RuleNotApplicable);
    };

    let Expr::SATInt(_, _, _, (denom_min, denom_max)) = denom.as_ref() else {
        return Err(RuleNotApplicable);
    };

    let mut candidates = Vec::new();

    let mut denom_candidates = Vec::new();
    if (*denom_min < 1) && (1 < *denom_max) {
        denom_candidates.push(1.0);
    }
    if (*denom_min < -1) && (-1 < *denom_max) {
        denom_candidates.push(-1.0);
    }
    if *denom_min != 0 {
        denom_candidates.push(f64::from(*denom_min));
    }
    if *denom_max != 0 {
        denom_candidates.push(f64::from(*denom_max));
    }
    for numer in [f64::from(*numer_min), f64::from(*numer_max)] {
        for denom in &denom_candidates {
            candidates.push(numer.div(denom).floor() as i32);
        }
    }

    let min = *candidates.iter().min().unwrap();
    let max = *candidates.iter().max().unwrap();

    let binding =
        validate_log_int_operands(vec![numer.as_ref().clone(), denom.as_ref().clone()], None)?;
    let [numer_bits, denom_bits] = binding.as_slice() else {
        return Err(RuleNotApplicable);
    };

    let bit_count = numer_bits.len();

    let mut new_symbols = symbols.clone();
    let mut new_clauses = vec![];

    let (quotient, remainder, sign_bit, _numer_sign, _abs_denom) =
        tseytin_divmod(numer_bits, denom_bits, &mut new_clauses, &mut new_symbols);

    let minus_quotient = tseytin_negate(
        &quotient.clone(),
        bit_count,
        &mut new_clauses,
        &mut new_symbols,
    );

    let minus_quotient_plus_one = tseytin_negate(
        &tseytin_add_two_power(
            &quotient.clone(),
            0,
            bit_count,
            &mut new_clauses,
            &mut new_symbols,
        ),
        bit_count,
        &mut new_clauses,
        &mut new_symbols,
    );

    let quotient_if_signs_differ = tseytin_select_array(
        tseytin_or(&remainder, &mut new_clauses, &mut new_symbols),
        &minus_quotient,
        &minus_quotient_plus_one,
        &mut new_clauses,
        &mut new_symbols,
    );

    let mut out = tseytin_select_array(
        sign_bit,
        &quotient,
        &quotient_if_signs_differ,
        &mut new_clauses,
        &mut new_symbols,
    );

    let new_bit_count = bit_magnitude(min).max(bit_magnitude(max));
    out.truncate(new_bit_count);

    Ok(Reduction::cnf(
        Expr::SATInt(
            Metadata::new(),
            SATIntEncoding::Log,
            Moo::new(into_matrix_expr!(out)),
            (min, max),
        ),
        new_clauses,
        new_symbols,
    ))
}

/// Shared restoring-division core. Returns (quotient, remainder, sign_bit (quotient sign), denom_sign, |denom|).
fn tseytin_divmod(
    // Using "Restoring division" algorithm
    // https://en.wikipedia.org/wiki/Division_algorithm#Restoring_division
    numer_bits: &[Expr],
    denom_bits: &[Expr],
    clauses: &mut Vec<CnfClause>,
    symbols: &mut SymbolTable,
) -> (Vec<Expr>, Vec<Expr>, Expr, Expr, Vec<Expr>) {
    let bit_count = numer_bits.len();

    let mut quotient = vec![false.into(); bit_count];

    let minus_numer = tseytin_negate(&numer_bits.to_vec(), bit_count, clauses, symbols);
    let minus_denom = tseytin_negate(&denom_bits.to_vec(), bit_count, clauses, symbols);

    // original sign bits
    let numer_sign = numer_bits[bit_count - 1].clone();
    let denom_sign = denom_bits[bit_count - 1].clone();

    let sign_bit = tseytin_xor(numer_sign.clone(), denom_sign.clone(), clauses, symbols);

    let abs_numer = tseytin_select_array(numer_sign, numer_bits, &minus_numer, clauses, symbols);
    let abs_denom = tseytin_select_array(
        denom_sign.clone(),
        denom_bits,
        &minus_denom,
        clauses,
        symbols,
    );

    let mut r = abs_numer;
    r.extend(std::iter::repeat_n(r[bit_count - 1].clone(), bit_count));
    let mut d = std::iter::repeat_n(false.into(), bit_count).collect_vec();
    d.extend(abs_denom.clone());

    let minus_d = tseytin_negate(&d.clone(), 2 * bit_count, clauses, symbols);
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
            clauses,
            symbols,
        );

        // q[i] = inverse of sign bit - 1 if positive, 0 if negative
        quotient[i] = tseytin_not(rminusd[2 * bit_count - 1].clone(), clauses, symbols);

        for j in 0..(2 * bit_count) {
            r[j] = tseytin_mux(
                quotient[i].clone(),
                r[j].clone(),       // use r if negative
                rminusd[j].clone(), // use r-d if positive
                clauses,
                symbols,
            );
        }
    }

    (quotient, r, sign_bit, denom_sign, abs_denom)
}

/// Converts SafeMod of SATInts to a single SATInt
///
/// ```text
/// SafeMod(SATInt(a), SATInt(b)) ~> SATInt(c)
///
/// ```
#[register_rule(("SAT", 4700))]
fn cnf_int_safemod(expr: &Expr, symbols: &SymbolTable) -> ApplicationResult {
    let Expr::SafeMod(_, numer, denom) = expr else {
        return Err(RuleNotApplicable);
    };

    let Expr::SATInt(_, _, _, (numer_min, numer_max)) = numer.as_ref() else {
        return Err(RuleNotApplicable);
    };

    let Expr::SATInt(_, _, _, (denom_min, denom_max)) = denom.as_ref() else {
        return Err(RuleNotApplicable);
    };

    // Determine range of result
    let b: i32 = cmp::max(denom_min.abs(), denom_max.abs());

    let mut min = 0;
    let mut max = 0;

    if *numer_min < 0 && 0 < *numer_max {
        min = 1 - b;
        max = b - 1;
    } else if *numer_min >= 0 {
        min = 0;
        max = b - 1;
    } else if *numer_max <= 0 {
        min = 1 - b;
        max = 0;
    }

    let binding =
        validate_log_int_operands(vec![numer.as_ref().clone(), denom.as_ref().clone()], None)?;
    let [numer_bits, denom_bits] = binding.as_slice() else {
        return Err(RuleNotApplicable);
    };

    let mut new_symbols = symbols.clone();
    let mut new_clauses = vec![];

    let (_quotient, full_remainder, sign_bit, denom_sign, abs_denom) =
        tseytin_divmod(numer_bits, denom_bits, &mut new_clauses, &mut new_symbols);

    let new_bit_count = bit_magnitude(min).max(bit_magnitude(max));

    // The restoring-division algorithm uses a 2*bit_count wide "r" register.
    // The final remainder is stored in the upper half of that register.
    let bit_count = numer_bits.len();
    let remainder: Vec<Expr> = full_remainder
        .iter()
        .skip(bit_count)
        .take(new_bit_count)
        .cloned()
        .collect();

    let minus_remainder = tseytin_negate(
        &remainder.clone(),
        new_bit_count,
        &mut new_clauses,
        &mut new_symbols,
    );

    let denom_minus_remainder = tseytin_int_adder(
        &abs_denom,
        &minus_remainder,
        new_bit_count,
        &mut new_clauses,
        &mut new_symbols,
    );

    let subtract_condition = tseytin_and(
        &vec![
            sign_bit,
            tseytin_or(&remainder.clone(), &mut new_clauses, &mut new_symbols),
        ],
        &mut new_clauses,
        &mut new_symbols,
    );

    let pos_out = tseytin_select_array(
        subtract_condition,
        &remainder,
        &denom_minus_remainder,
        &mut new_clauses,
        &mut new_symbols,
    );

    let neg_out = tseytin_negate(&pos_out, new_bit_count, &mut new_clauses, &mut new_symbols);

    let out = tseytin_select_array(
        denom_sign,
        &pos_out,
        &neg_out,
        &mut new_clauses,
        &mut new_symbols,
    );

    Ok(Reduction::cnf(
        Expr::SATInt(
            Metadata::new(),
            SATIntEncoding::Log,
            Moo::new(into_matrix_expr!(out)),
            (min, max),
        ),
        new_clauses,
        new_symbols,
    ))
}

/// Converts SafePow of SATInts to a single SATInt
///
/// ```text
/// SafePow(SATInt(a), SATInt(b)) ~> SATInt(c)
///
/// ```
#[register_rule(("SAT", 4700))]
fn cnf_int_safepow(expr: &Expr, symbols: &SymbolTable) -> ApplicationResult {
    // use 'Exponentiation by squaring'
    // TODO: Split arithemetic logic into its own method
    let Expr::SafePow(_, base, exp) = expr else {
        return Err(RuleNotApplicable);
    };

    let Expr::SATInt(_, _, _, (bmin, bmax)) = base.as_ref() else {
        return Err(RuleNotApplicable);
    };

    let Expr::SATInt(_, _, _, (emin, emax)) = exp.as_ref() else {
        return Err(RuleNotApplicable);
    };

    // if exponent cannot be positive this has no return value
    if *emax < 0 {
        return Err(RuleNotApplicable);
    }

    let rmin;
    let mut rmax;
    if *bmin < 0 {
        // minimum is "minimum base" ^ "highest odd power"
        rmin = i32::pow(*bmin, (if emax % 2 == 0 { emax - 1 } else { *emax } as u32));

        // maximum is max("minimum base" ^ "highest even power", "maximum base" ^ "highest power")
        rmax = i32::pow(*bmin, (if emax % 2 == 0 { *emax } else { emax - 1 } as u32));
        rmax = rmax.max(i32::pow(*bmax, *emax as u32));
    } else {
        rmin = i32::pow(*bmin, *emin.max(&0) as u32);
        rmax = i32::pow(*bmax, *emax as u32);
    }

    validate_log_int_operands(vec![base.as_ref().clone()], None)?;

    let binding = validate_log_int_operands(vec![exp.as_ref().clone()], None)?;
    let [exp_bits] = binding.as_slice() else {
        return Err(RuleNotApplicable);
    };

    let mut new_symbols = symbols.clone();
    let mut new_clauses = vec![];

    let mut powers = vec![base.as_ref().clone()];

    // can ignore final (sign) bit as safepow ensures the exponent is positive
    for _ in 0..exp_bits.len() - 2 {
        let operand = powers.last().unwrap();
        powers.push(log_square(operand, &mut new_clauses, &mut new_symbols));
    }

    let mut result = int_to_log(1);

    for i in 0..exp_bits.len() - 1 {
        let mux_power = log_select(
            &exp_bits[i],
            &int_to_log(1),
            &powers[i],
            &mut new_clauses,
            &mut new_symbols,
        );
        result = log_multiply(
            &result.clone(),
            &mux_power,
            &mut new_clauses,
            &mut new_symbols,
        );
    }

    result = satint_set_range(result, rmin, rmax);
    result = log_minimize_bits(result);

    Ok(Reduction::cnf(result, new_clauses, new_symbols))
}

/// Converts allDiff of SATInts to a boolean
///
/// ```text
/// allDiff(SATInt(a), SATInt(b), ...) ~> bool
///
/// ```
#[register_rule(("SAT", 4600))]
fn sat_log_alldiff(expr: &Expr, symbols: &SymbolTable) -> ApplicationResult {
    let Expr::AllDiff(_, moo_operands) = expr else {
        return Err(RuleNotApplicable);
    };
    let operands = moo_operands.as_ref().clone().unwrap_matrix_unchecked().unwrap().0;

    validate_log_int_operands(operands.clone(), None)?;

    let mut new_symbols = symbols.clone();
    let mut new_clauses = vec![];

    let mut cum = true.into();

    // go through every possible pairwise combination and check for inequality
    for (i, a) in operands.iter().enumerate() {
        for b in operands.iter().skip(i + 1) {
            cum = tseytin_and(
                &vec![cum, log_neq(a, b, &mut new_clauses, &mut new_symbols)],
                &mut new_clauses,
                &mut new_symbols,
            );
        }
    }

    Ok(Reduction::cnf(cum, new_clauses, new_symbols))
}


/// Compare the equality of two log SATInts
pub fn log_neq(
    x: &Expr,
    y: &Expr,
    clauses: &mut Vec<CnfClause>,
    symbols: &mut SymbolTable,
) -> Expr {
    let Expr::SATInt(_, SATIntEncoding::Log, moo_x_bits, (xmin, xmax)) = x else {
        panic!("Log int should always be used here");
    };

    let Expr::SATInt(_, SATIntEncoding::Log, moo_y_bits, (ymin, ymax)) = y else {
        panic!("Log int should always be used here");
    };

    // if domains do not intersect then two integers cannot be equal
    if xmax < ymin || ymax < xmin {
        return true.into();
    }

    let x_bits = moo_x_bits.as_ref().clone().unwrap_list().unwrap();
    let y_bits = moo_y_bits.as_ref().clone().unwrap_list().unwrap();

    let min_len = x_bits.len().min(y_bits.len());

    let mut cum = tseytin_xor(x_bits[0].clone(), y_bits[0].clone(), clauses, symbols);
    for i in 1..min_len {
        cum = tseytin_or(
            &vec![
                cum,
                tseytin_xor(x_bits[i].clone(), y_bits[i].clone(), clauses, symbols),
            ],
            clauses,
            symbols,
        );
    }

    if x_bits.len() < y_bits.len() {
        let sign = x_bits.last().unwrap();
        for bit in y_bits.into_iter().skip(min_len) {
            cum = tseytin_or(
                &vec![cum, tseytin_xor(sign.clone(), bit, clauses, symbols)],
                clauses,
                symbols,
            );
        }
    } else if x_bits.len() > y_bits.len() {
        let sign = y_bits.last().unwrap();
        for bit in x_bits.into_iter().skip(min_len) {
            cum = tseytin_or(
                &vec![cum, tseytin_xor(sign.clone(), bit, clauses, symbols)],
                clauses,
                symbols,
            );
        }
    }

    return cum;
}
