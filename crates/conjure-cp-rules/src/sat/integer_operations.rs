use conjure_cp::ast::Expression as Expr;
use conjure_cp::ast::SymbolTable;
use conjure_cp::rule_engine::{
    ApplicationError::RuleNotApplicable, ApplicationResult, Reduction,
    register_rule,
};


use conjure_cp::ast::AbstractLiteral::Matrix;
use conjure_cp::ast::Metadata;
use conjure_cp::ast::Moo;
use conjure_cp::into_matrix_expr;

use itertools::Itertools;

use super::integer_repr::{validate_sat_int_operands, BITS};
use super::boolean::{tseytin_and,
tseytin_iff,
tseytin_imply,
tseytin_mux,
tseytin_not,
tseytin_or,
tseytin_xor};

/// Converts an inequality expression between two SATInts to a boolean expression in cnf.
///
/// ```text
/// SATInt(a) </>/<=/>= SATInt(b) ~> Bool
///
/// ```
#[register_rule(("SAT", 4100))]
fn cnf_int_ineq(expr: &Expr, symbols: &SymbolTable) -> ApplicationResult {
    let (lhs, rhs, strict) = match expr {
        Expr::Lt(_, x, y) => (y, x, true),
        Expr::Gt(_, x, y) => (x, y, true),
        Expr::Leq(_, x, y) => (y, x, false),
        Expr::Geq(_, x, y) => (x, y, false),
        _ => return Err(RuleNotApplicable),
    };

    let binding = validate_sat_int_operands(vec![lhs.as_ref().clone(), rhs.as_ref().clone()])?;
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
#[register_rule(("SAT", 9100))]
fn cnf_int_eq(expr: &Expr, symbols: &SymbolTable) -> ApplicationResult {
    let Expr::Eq(_, lhs, rhs) = expr else {
        return Err(RuleNotApplicable);
    };

    let binding = validate_sat_int_operands(vec![lhs.as_ref().clone(), rhs.as_ref().clone()])?;
    let [lhs_bits, rhs_bits] = binding.as_slice() else {
        return Err(RuleNotApplicable);
    };

    let mut output = true.into();
    let mut new_symbols = symbols.clone();
    let mut new_clauses = vec![];
    let mut comparison;

    for i in 0..BITS {
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
#[register_rule(("SAT", 4100))]
fn cnf_int_neq(expr: &Expr, symbols: &SymbolTable) -> ApplicationResult {
    let Expr::Neq(_, lhs, rhs) = expr else {
        return Err(RuleNotApplicable);
    };

    let binding = validate_sat_int_operands(vec![lhs.as_ref().clone(), rhs.as_ref().clone()])?;
    let [lhs_bits, rhs_bits] = binding.as_slice() else {
        return Err(RuleNotApplicable);
    };

    let mut output = false.into();
    let mut new_symbols = symbols.clone();
    let mut new_clauses = vec![];
    let mut comparison;

    for i in 0..BITS {
        comparison = tseytin_xor(
            lhs_bits[i].clone(),
            rhs_bits[i].clone(),
            &mut new_clauses,
            &mut new_symbols,
        );
        output = tseytin_or(
            &vec![comparison, output],
            &mut new_clauses,
            &mut new_symbols,
        );
    }

    Ok(Reduction::cnf(output, new_clauses, new_symbols))
}

// Creates a boolean expression for > or >=
// a > b or a >= b
// This can also be used for < and <= by reversing the order of the inputs
// Returns result, new symbol table, new clauses
fn inequality_boolean(
    a: Vec<Expr>,
    b: Vec<Expr>,
    strict: bool,
    clauses: &mut Vec<Expr>,
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

    let mut lhs;
    let mut rhs;
    let mut iff;
    for n in 1..(BITS - 1) {
        notb = tseytin_not(b[n].clone(), clauses, symbols);
        lhs = tseytin_and(&vec![a[n].clone(), notb.clone()], clauses, symbols);
        iff = tseytin_iff(a[n].clone(), b[n].clone(), clauses, symbols);
        rhs = tseytin_and(&vec![iff.clone(), output.clone()], clauses, symbols);
        output = tseytin_or(&vec![lhs.clone(), rhs.clone()], clauses, symbols);
    }

    // final bool is the sign bit and should be handled inversely
    let nota = tseytin_not(a[BITS - 1].clone(), clauses, symbols);
    lhs = tseytin_and(&vec![nota, b[BITS - 1].clone()], clauses, symbols);
    iff = tseytin_iff(a[BITS - 1].clone(), b[BITS - 1].clone(), clauses, symbols);
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
#[register_rule(("SAT", 4100))]
fn cnf_int_sum(expr: &Expr, symbols: &SymbolTable) -> ApplicationResult {
    let Expr::Sum(_, exprs) = expr else {
        return Err(RuleNotApplicable);
    };

    let Expr::AbstractLiteral(_, Matrix(exprs_list, _)) = exprs.as_ref() else {
        return Err(RuleNotApplicable);
    };

    let mut exprs_bits = validate_sat_int_operands(exprs_list.clone())?;

    let mut new_symbols = symbols.clone();
    let mut values;
    let mut new_clauses = vec![];

    while exprs_bits.len() > 1 {
        let mut next = Vec::with_capacity(exprs_bits.len().div_ceil(2));
        let mut iter = exprs_bits.into_iter();

        while let Some(a) = iter.next() {
            if let Some(b) = iter.next() {
                values = tseytin_int_adder(&a, &b, BITS, &mut new_clauses, &mut new_symbols);
                next.push(values);
            } else {
                next.push(a);
            }
        }

        exprs_bits = next;
    }

    let result = exprs_bits.pop().unwrap();

    Ok(Reduction::cnf(
        Expr::SATInt(Metadata::new(), Moo::new(into_matrix_expr!(result))),
        new_clauses,
        new_symbols,
    ))
}

// Returns result, new symbol table, new clauses
fn tseytin_int_adder(
    x: &[Expr],
    y: &[Expr],
    bits: usize,
    clauses: &mut Vec<Expr>,
    symbols: &mut SymbolTable,
) -> Vec<Expr> {
    //TODO Optimizing for constants
    let (mut result, mut carry) = tseytin_half_adder(x[0].clone(), y[0].clone(), clauses, symbols);

    let mut output = vec![result];
    for i in 1..bits {
        (result, carry) =
            tseytin_full_adder(x[i].clone(), y[i].clone(), carry.clone(), clauses, symbols);
        output.push(result);
    }

    output
}

// Returns: result, carry, new symbol table, new clauses
fn tseytin_full_adder(
    a: Expr,
    b: Expr,
    carry: Expr,
    clauses: &mut Vec<Expr>,
    symbols: &mut SymbolTable,
) -> (Expr, Expr) {
    let axorb = tseytin_xor(a.clone(), b.clone(), clauses, symbols);
    let result = tseytin_xor(axorb.clone(), carry.clone(), clauses, symbols);
    let aandb = tseytin_and(&vec![a, b], clauses, symbols);
    let carryandaxorb = tseytin_and(&vec![carry, axorb], clauses, symbols);
    let carryout = tseytin_or(&vec![aandb, carryandaxorb], clauses, symbols);

    (result, carryout)
}

fn tseytin_half_adder(
    a: Expr,
    b: Expr,
    clauses: &mut Vec<Expr>,
    symbols: &mut SymbolTable,
) -> (Expr, Expr) {
    let result = tseytin_xor(a.clone(), b.clone(), clauses, symbols);
    let carry = tseytin_and(&vec![a, b], clauses, symbols);

    (result, carry)
}

/// this function is for specifically adding a power of two constant to a cnf int
fn tseytin_add_two_power(
    expr: &[Expr],
    exponent: usize,
    bits: usize,
    clauses: &mut Vec<Expr>,
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

// Returns result, new symbol table, new clauses
fn cnf_shift_add_multiply(
    x: &[Expr],
    y: &[Expr],
    bits: usize,
    clauses: &mut Vec<Expr>,
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
        x_0andy_i = tseytin_and(&vec![x[0].clone(), bit.clone()], clauses, symbols);
        s.push(x_0andy_i);
    }

    let mut sum;
    let mut if_true;
    let mut not_x_n;
    let mut if_false;

    for item in x.iter().take(bits).skip(1) {
        // y << 1
        for i in (1..bits * 2).rev() {
            y[i] = y[i - 1].clone();
        }
        y[0] = false.into();

        // TODO switch to multiplexer
        sum = tseytin_int_adder(&s, &y, bits * 2, clauses, symbols);
        not_x_n = tseytin_not(item.clone(), clauses, symbols);

        for i in 0..(bits * 2) {
            if_true = tseytin_and(&vec![item.clone(), sum[i].clone()], clauses, symbols);
            if_false = tseytin_and(&vec![not_x_n.clone(), s[i].clone()], clauses, symbols);
            s[i] = tseytin_or(&vec![if_true.clone(), if_false.clone()], clauses, symbols);
        }
    }

    //TODO: At the moment, this doesn't account for overflows (perhaps this could use a bubble in the future?)
    s[..bits].to_vec()
}

/// Converts product of SATInts to a single SATInt
///
/// ```text
/// Product(SATInt(a), SATInt(b), ...) ~> SATInt(c)
///
/// ```
#[register_rule(("SAT", 9000))]
fn cnf_int_product(expr: &Expr, symbols: &SymbolTable) -> ApplicationResult {
    let Expr::Product(_, exprs) = expr else {
        return Err(RuleNotApplicable);
    };

    let Expr::AbstractLiteral(_, Matrix(exprs_list, _)) = exprs.as_ref() else {
        return Err(RuleNotApplicable);
    };

    let mut exprs_bits = validate_sat_int_operands(exprs_list.clone())?;

    let mut new_symbols = symbols.clone();
    let mut values;
    let mut new_clauses = vec![];

    while exprs_bits.len() > 1 {
        let mut next = Vec::with_capacity(exprs_bits.len().div_ceil(2));
        let mut iter = exprs_bits.into_iter();

        while let Some(a) = iter.next() {
            if let Some(b) = iter.next() {
                values = cnf_shift_add_multiply(&a, &b, 8, &mut new_clauses, &mut new_symbols);
                next.push(values);
            } else {
                next.push(a);
            }
        }

        exprs_bits = next;
    }

    let result = exprs_bits.pop().unwrap();

    Ok(Reduction::cnf(
        Expr::SATInt(Metadata::new(), Moo::new(into_matrix_expr!(result))),
        new_clauses,
        new_symbols,
    ))
}

/// Converts negation of a SATInt to a SATInt
///
/// ```text
/// -SATInt(a) ~> SATInt(b)
///
/// ```
#[register_rule(("SAT", 4100))]
fn cnf_int_neg(expr: &Expr, symbols: &SymbolTable) -> ApplicationResult {
    let Expr::Neg(_, expr) = expr else {
        return Err(RuleNotApplicable);
    };

    let binding = validate_sat_int_operands(vec![expr.as_ref().clone()])?;
    let [bits] = binding.as_slice() else {
        return Err(RuleNotApplicable);
    };
    let mut new_clauses = vec![];
    let mut new_symbols = symbols.clone();

    let result = tseytin_negate(bits, BITS, &mut new_clauses, &mut new_symbols);

    Ok(Reduction::cnf(
        Expr::SATInt(Metadata::new(), Moo::new(into_matrix_expr!(result))),
        new_clauses,
        new_symbols,
    ))
}

fn tseytin_negate(
    expr: &Vec<Expr>,
    bits: usize,
    clauses: &mut Vec<Expr>,
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
#[register_rule(("SAT", 4100))]
fn cnf_int_min(expr: &Expr, symbols: &SymbolTable) -> ApplicationResult {
    let Expr::Min(_, exprs) = expr else {
        return Err(RuleNotApplicable);
    };

    let Expr::AbstractLiteral(_, Matrix(exprs_list, _)) = exprs.as_ref() else {
        return Err(RuleNotApplicable);
    };

    let mut exprs_bits = validate_sat_int_operands(exprs_list.clone())?;

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
        Expr::SATInt(Metadata::new(), Moo::new(into_matrix_expr!(result))),
        new_clauses,
        new_symbols,
    ))
}

fn tseytin_binary_min_max(
    x: &[Expr],
    y: &[Expr],
    min: bool,
    clauses: &mut Vec<Expr>,
    symbols: &mut SymbolTable,
) -> Vec<Expr> {
    let mut out = vec![];

    for i in 0..BITS {
        out.push(tseytin_xor(x[i].clone(), y[i].clone(), clauses, symbols))
    }

    // TODO: compare generated expression to using MUX

    let mask = if min {
        // mask is 1 if x > y
        inequality_boolean(x.to_owned(), y.to_owned(), true, clauses, symbols)
    } else {
        // flip the args if getting maximum x < y -> 1
        inequality_boolean(y.to_owned(), x.to_owned(), true, clauses, symbols)
    };

    for item in out.iter_mut().take(BITS) {
        *item = tseytin_and(&vec![item.clone(), mask.clone()], clauses, symbols);
    }

    for i in 0..BITS {
        out[i] = tseytin_xor(x[i].clone(), out[i].clone(), clauses, symbols);
    }

    out
}

/// Converts max of SATInts to a single SATInt
///
/// ```text
/// Max(SATInt(a), SATInt(b), ...) ~> SATInt(c)
///
/// ```
#[register_rule(("SAT", 4100))]
fn cnf_int_max(expr: &Expr, symbols: &SymbolTable) -> ApplicationResult {
    let Expr::Max(_, exprs) = expr else {
        return Err(RuleNotApplicable);
    };

    let Expr::AbstractLiteral(_, Matrix(exprs_list, _)) = exprs.as_ref() else {
        return Err(RuleNotApplicable);
    };

    let mut exprs_bits = validate_sat_int_operands(exprs_list.clone())?;

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
        Expr::SATInt(Metadata::new(), Moo::new(into_matrix_expr!(result))),
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
#[register_rule(("SAT", 4100))]
fn cnf_int_abs(expr: &Expr, symbols: &SymbolTable) -> ApplicationResult {
    let Expr::Abs(_, expr) = expr else {
        return Err(RuleNotApplicable);
    };

    let binding = validate_sat_int_operands(vec![expr.as_ref().clone()])?;
    let [bits] = binding.as_slice() else {
        return Err(RuleNotApplicable);
    };
    let mut new_clauses = vec![];
    let mut new_symbols = symbols.clone();

    let mut result = vec![];

    // invert bits
    for bit in bits {
        result.push(tseytin_not(bit.clone(), &mut new_clauses, &mut new_symbols));
    }

    // add one
    result = tseytin_add_two_power(&result, 0, BITS, &mut new_clauses, &mut new_symbols);

    for i in 0..BITS {
        result[i] = tseytin_mux(
            bits[BITS - 1].clone(),
            bits[i].clone(),
            result[i].clone(),
            &mut new_clauses,
            &mut new_symbols,
        )
    }

    Ok(Reduction::cnf(
        Expr::SATInt(Metadata::new(), Moo::new(into_matrix_expr!(result))),
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
#[register_rule(("SAT", 4100))]
fn cnf_int_safediv(expr: &Expr, symbols: &SymbolTable) -> ApplicationResult {
    // Using "Restoring division" algorithm
    // https://en.wikipedia.org/wiki/Division_algorithm#Restoring_division
    let Expr::SafeDiv(_, numer, denom) = expr else {
        return Err(RuleNotApplicable);
    };

    let binding = validate_sat_int_operands(vec![numer.as_ref().clone(), denom.as_ref().clone()])?;
    let [numer_bits, denom_bits] = binding.as_slice() else {
        return Err(RuleNotApplicable);
    };

    // TODO: Separate into division/mod function
    // TODO: Support negatives

    let mut new_symbols = symbols.clone();
    let mut new_clauses = vec![];
    let mut quotient = vec![false.into(); BITS];

    let mut r = numer_bits.clone();
    r.extend(std::iter::repeat_n(r[BITS - 1].clone(), BITS));
    let mut d = std::iter::repeat_n(false.into(), BITS).collect_vec();
    d.extend(denom_bits.clone());

    let minus_d = tseytin_negate(&d.clone(), 2 * BITS, &mut new_clauses, &mut new_symbols);
    let mut rminusd;

    for i in (0..BITS).rev() {
        // r << 1
        for j in (1..BITS * 2).rev() {
            r[j] = r[j - 1].clone();
        }
        r[0] = false.into();

        rminusd = tseytin_int_adder(
            &r.clone(),
            &minus_d.clone(),
            2 * BITS,
            &mut new_clauses,
            &mut new_symbols,
        );

        // TODO: For mod don't calculate on final iter
        quotient[i] = tseytin_not(
            // q[i] = inverse of sign bit - 1 if positive, 0 if negative
            rminusd[2 * BITS - 1].clone(),
            &mut new_clauses,
            &mut new_symbols,
        );

        // TODO: For div don't calculate on final iter
        for j in 0..(2 * BITS) {
            r[j] = tseytin_mux(
                quotient[i].clone(),
                r[j].clone(),       // use r if negative
                rminusd[j].clone(), // use r-d if positive
                &mut new_clauses,
                &mut new_symbols,
            );
        }
    }

    Ok(Reduction::cnf(
        Expr::SATInt(Metadata::new(), Moo::new(into_matrix_expr!(quotient))),
        new_clauses,
        new_symbols,
    ))
}

/*
/// Converts SafeMod of SATInts to a single SATInt
///
/// ```text
/// SafeMod(SATInt(a), SATInt(b)) ~> SATInt(c)
///
/// ```
#[register_rule(("SAT", 4100))]
fn cnf_int_safemod(expr: &Expr, _: &SymbolTable) -> ApplicationResult {}

/// Converts SafePow of SATInts to a single SATInt
///
/// ```text
/// SafePow(SATInt(a), SATInt(b)) ~> SATInt(c)
///
/// ```
#[register_rule(("SAT", 4100))]
fn cnf_int_safepow(expr: &Expr, _: &SymbolTable) -> ApplicationResult {
    // use 'Exponentiation by squaring'
}
*/
