use conjure_cp::ast::Expression as Expr;
use conjure_cp::ast::SymbolTable;
use conjure_cp::rule_engine::{
    ApplicationError, ApplicationError::RuleNotApplicable, ApplicationResult, Reduction,
    register_rule,
};

use conjure_cp::ast::AbstractLiteral::Matrix;
use conjure_cp::ast::Metadata;
use conjure_cp::ast::{Atom, Domain, Literal, Moo, Range};
use conjure_cp::into_matrix_expr;

use conjure_cp::essence_expr;
use itertools::Itertools;

use crate::cnf::tseytin_and;
use crate::cnf::tseytin_iff;
use crate::cnf::tseytin_imply;
use crate::cnf::tseytin_mux;
use crate::cnf::tseytin_not;
use crate::cnf::tseytin_or;
use crate::cnf::tseytin_xor;

// The number of bits used to represent the integer.
// This is a fixed value for the representation, but could be made dynamic if needed.
const BITS: usize = 8;

#[register_rule(("CNF", 9500))]
fn integer_decision_representation(expr: &Expr, symbols: &SymbolTable) -> ApplicationResult {
    // thing we are representing must be a reference
    let Expr::Atomic(_, Atom::Reference(name)) = expr else {
        return Err(RuleNotApplicable);
    };

    // thing we are representing must be a variable
    // symbols
    //     .lookup(name)
    //     .ok_or(RuleNotApplicable)?
    //     .as_var()
    //     .ok_or(RuleNotApplicable)?;

    // thing we are representing must be an integer
    let Domain::Int(ranges) = name.domain().unwrap() else {
        return Err(RuleNotApplicable);
    };

    let mut symbols = symbols.clone();

    let new_name = &name.name().to_owned();

    let repr_exists = symbols
        .get_representation(new_name, &["int_to_atom"])
        .is_some();

    let representation = symbols
        .get_or_add_representation(new_name, &["int_to_atom"])
        .ok_or(RuleNotApplicable)?;

    let bits = representation[0]
        .clone()
        .expression_down(&symbols)?
        .into_iter()
        .map(|(_, expr)| expr.clone())
        .collect();

    let cnf_int = Expr::CnfInt(Metadata::new(), Moo::new(into_matrix_expr!(bits)));

    if !repr_exists {
        // add domain ranges as constraints if this is the first time the representation is added
        Ok(Reduction::new(
            cnf_int.clone(),
            vec![int_domain_to_expr(cnf_int.clone(), &ranges)], // contains domain rules
            symbols,
        ))
    } else {
        Ok(Reduction::pure(cnf_int.clone()))
    }
}

#[register_rule(("CNF", 9500))]
fn literal_cnf_int(expr: &Expr, _: &SymbolTable) -> ApplicationResult {
    let mut value = {
        if let Expr::Atomic(_, Atom::Literal(Literal::Int(v))) = expr {
            *v
        } else {
            return Err(RuleNotApplicable);
        }
    };

    //TODO: add support for negatives

    let mut binary_encoding = vec![];

    for _ in 0..BITS {
        binary_encoding.push(Expr::Atomic(
            Metadata::new(),
            Atom::Literal(Literal::Bool((value & 1) != 0)),
        ));
        value >>= 1;
    }

    Ok(Reduction::pure(Expr::CnfInt(
        Metadata::new(),
        Moo::new(into_matrix_expr!(binary_encoding)),
    )))
}

/// This function takes a target expression and a vector of ranges and creates an expression representing the ranges with the target expression as the subject
///
/// E.g. x : int(4), int(10..20), int(30..) ~~> Or(x=4, 10<=x<=20, x>=30)
fn int_domain_to_expr(subject: Expr, ranges: &Vec<Range<i32>>) -> Expr {
    let mut output = vec![];

    let value = Moo::new(subject);

    for range in ranges {
        match range {
            Range::Single(x) => output.push(essence_expr!(&value = &x)),
            Range::Bounded(x, y) => output.push(essence_expr!("&value >= &x /\\ &value <= &y")),
            Range::UnboundedR(x) => output.push(essence_expr!(&value >= &x)),
            Range::UnboundedL(x) => output.push(essence_expr!(&value <= &x)),
        }
    }

    Expr::Or(Metadata::new(), Moo::new(into_matrix_expr!(output)))
}

/// Converts an inequality expression between two CnfInts to a boolean expression in cnf.
///
/// ```text
/// CnfInt(a) </>/<=/>= CnfInt(b) ~> Bool
///
/// ```
#[register_rule(("CNF", 4100))]
fn cnf_int_ineq(expr: &Expr, symbols: &SymbolTable) -> ApplicationResult {
    let (lhs, rhs, strict) = match expr {
        Expr::Lt(_, x, y) => (y, x, true),
        Expr::Gt(_, x, y) => (x, y, true),
        Expr::Leq(_, x, y) => (y, x, false),
        Expr::Geq(_, x, y) => (x, y, false),
        _ => return Err(RuleNotApplicable),
    };

    let binding = validate_cnf_int_operands(vec![lhs.as_ref().clone(), rhs.as_ref().clone()])?;
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

/// This function confirms that all of the input expressions are CnfInts, and returns vectors for each input of their bits
fn validate_cnf_int_operands(exprs: Vec<Expr>) -> Result<Vec<Vec<Expr>>, ApplicationError> {
    let out: Result<Vec<Vec<_>>, _> = exprs
        .clone()
        .into_iter()
        .map(|expr| {
            let Expr::CnfInt(_, inner) = expr else {
                return Err(RuleNotApplicable);
            };
            let Some(bits) = inner.as_ref().clone().unwrap_list() else {
                return Err(RuleNotApplicable);
            };
            Ok(bits)
        })
        .collect();

    out
}

/// Converts a `=` expression between two CnfInts to a boolean expression in cnf
///
/// ```text
/// CnfInt(a) = CnfInt(b) ~> Bool
///
/// ```
#[register_rule(("CNF", 9100))]
fn cnf_int_eq(expr: &Expr, symbols: &SymbolTable) -> ApplicationResult {
    let Expr::Eq(_, lhs, rhs) = expr else {
        return Err(RuleNotApplicable);
    };

    let binding = validate_cnf_int_operands(vec![lhs.as_ref().clone(), rhs.as_ref().clone()])?;
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

/// Converts a != expression between two CnfInts to a boolean expression in cnf
///
/// ```text
/// CnfInt(a) != CnfInt(b) ~> Bool
///
/// ```
#[register_rule(("CNF", 4100))]
fn cnf_int_neq(expr: &Expr, symbols: &SymbolTable) -> ApplicationResult {
    let Expr::Neq(_, lhs, rhs) = expr else {
        return Err(RuleNotApplicable);
    };

    let binding = validate_cnf_int_operands(vec![lhs.as_ref().clone(), rhs.as_ref().clone()])?;
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
    let nota;
    nota = tseytin_not(a[BITS - 1].clone(), clauses, symbols);
    lhs = tseytin_and(&vec![nota, b[BITS - 1].clone()], clauses, symbols);
    iff = tseytin_iff(a[BITS - 1].clone(), b[BITS - 1].clone(), clauses, symbols);
    rhs = tseytin_and(&vec![iff.clone(), output.clone()], clauses, symbols);
    output = tseytin_or(&vec![lhs.clone(), rhs.clone()], clauses, symbols);

    output
}

/// Converts sum of CnfInts to a single CnfInt
///
/// ```text
/// Sum(CnfInt(a), CnfInt(b), ...) ~> CnfInt(c)
///
/// ```
#[register_rule(("CNF", 4100))]
fn cnf_int_sum(expr: &Expr, symbols: &SymbolTable) -> ApplicationResult {
    let Expr::Sum(_, exprs) = expr else {
        return Err(RuleNotApplicable);
    };

    let Expr::AbstractLiteral(_, Matrix(exprs_list, _)) = exprs.as_ref() else {
        return Err(RuleNotApplicable);
    };

    let mut exprs_bits = validate_cnf_int_operands(exprs_list.clone())?;

    let mut new_symbols = symbols.clone();
    let mut values;
    let mut new_clauses = vec![];

    while exprs_bits.len() > 1 {
        let mut next = Vec::with_capacity((exprs_bits.len() + 1) / 2);
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
        Expr::CnfInt(Metadata::new(), Moo::new(into_matrix_expr!(result))),
        new_clauses,
        new_symbols,
    ))
}

// Returns result, new symbol table, new clauses
fn tseytin_int_adder(
    x: &Vec<Expr>,
    y: &Vec<Expr>,
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
    expr: &Vec<Expr>,
    exponent: usize,
    bits: usize,
    clauses: &mut Vec<Expr>,
    symbols: &mut SymbolTable,
) -> Vec<Expr> {
    let mut result = vec![];
    let mut product = expr[exponent].clone();

    for i in 0..exponent {
        result.push(expr[i].clone());
    }

    result.push(tseytin_not(expr[exponent].clone(), clauses, symbols));

    for i in (exponent + 1)..bits {
        result.push(tseytin_xor(
            product.clone(),
            expr[i].clone(),
            clauses,
            symbols,
        ));
        product = tseytin_and(&vec![product, expr[i].clone()], clauses, symbols);
    }

    result
}

// Returns result, new symbol table, new clauses
fn cnf_shift_add_multiply(
    x: &Vec<Expr>,
    y: &Vec<Expr>,
    bits: usize,
    clauses: &mut Vec<Expr>,
    symbols: &mut SymbolTable,
) -> Vec<Expr> {
    let mut x = x.clone();
    let mut y = y.clone();

    //TODO Optimizing for constants
    //TODO Optimize addition for i left shifted values - skip first i bits

    // extend sign bits of operands to 2*`bits`
    x.extend(std::iter::repeat(x[bits - 1].clone()).take(bits));
    y.extend(std::iter::repeat(y[bits - 1].clone()).take(bits));

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

    for n in 1..bits {
        // y << 1
        for i in (1..bits * 2).rev() {
            y[i] = y[i - 1].clone();
        }
        y[0] = false.into();

        // TODO switch to multiplexer
        sum = tseytin_int_adder(&s, &y, bits * 2, clauses, symbols);
        not_x_n = tseytin_not(x[n].clone(), clauses, symbols);

        for i in 0..(bits * 2) {
            if_true = tseytin_and(&vec![x[n].clone(), sum[i].clone()], clauses, symbols);
            if_false = tseytin_and(&vec![not_x_n.clone(), s[i].clone()], clauses, symbols);
            s[i] = tseytin_or(&vec![if_true.clone(), if_false.clone()], clauses, symbols);
        }
    }

    //TODO: At the moment, this doesn't account for overflows (perhaps this could use a bubble in the future?)
    s[..bits].to_vec()
}

/// Converts product of CnfInts to a single CnfInt
///
/// ```text
/// Product(CnfInt(a), CnfInt(b), ...) ~> CnfInt(c)
///
/// ```
#[register_rule(("CNF", 9000))]
fn cnf_int_product(expr: &Expr, symbols: &SymbolTable) -> ApplicationResult {
    let Expr::Product(_, exprs) = expr else {
        return Err(RuleNotApplicable);
    };

    let Expr::AbstractLiteral(_, Matrix(exprs_list, _)) = exprs.as_ref() else {
        return Err(RuleNotApplicable);
    };

    let mut exprs_bits = validate_cnf_int_operands(exprs_list.clone())?;

    let mut new_symbols = symbols.clone();
    let mut values;
    let mut new_clauses = vec![];

    while exprs_bits.len() > 1 {
        let mut next = Vec::with_capacity((exprs_bits.len() + 1) / 2);
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
        Expr::CnfInt(Metadata::new(), Moo::new(into_matrix_expr!(result))),
        new_clauses,
        new_symbols,
    ))
}

/// Converts negation of a CnfInt to a CnfInt
///
/// ```text
/// -CnfInt(a) ~> CnfInt(b)
///
/// ```
#[register_rule(("CNF", 4100))]
fn cnf_int_neg(expr: &Expr, symbols: &SymbolTable) -> ApplicationResult {
    let Expr::Neg(_, expr) = expr else {
        return Err(RuleNotApplicable);
    };

    let binding = validate_cnf_int_operands(vec![expr.as_ref().clone()])?;
    let [bits] = binding.as_slice() else {
        return Err(RuleNotApplicable);
    };
    let mut new_clauses = vec![];
    let mut new_symbols = symbols.clone();

    let result = tseytin_negate(&bits, BITS, &mut new_clauses, &mut new_symbols);

    Ok(Reduction::cnf(
        Expr::CnfInt(Metadata::new(), Moo::new(into_matrix_expr!(result))),
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

/// Converts min of CnfInts to a single CnfInt
///
/// ```text
/// Min(CnfInt(a), CnfInt(b), ...) ~> CnfInt(c)
///
/// ```
#[register_rule(("CNF", 4100))]
fn cnf_int_min(expr: &Expr, symbols: &SymbolTable) -> ApplicationResult {
    let Expr::Min(_, exprs) = expr else {
        return Err(RuleNotApplicable);
    };

    let Expr::AbstractLiteral(_, Matrix(exprs_list, _)) = exprs.as_ref() else {
        return Err(RuleNotApplicable);
    };

    let mut exprs_bits = validate_cnf_int_operands(exprs_list.clone())?;

    let mut new_symbols = symbols.clone();
    let mut values;
    let mut new_clauses = vec![];

    while exprs_bits.len() > 1 {
        let mut next = Vec::with_capacity((exprs_bits.len() + 1) / 2);
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
        Expr::CnfInt(Metadata::new(), Moo::new(into_matrix_expr!(result))),
        new_clauses,
        new_symbols,
    ))
}

fn tseytin_binary_min_max(
    x: &Vec<Expr>,
    y: &Vec<Expr>,
    min: bool,
    clauses: &mut Vec<Expr>,
    symbols: &mut SymbolTable,
) -> Vec<Expr> {
    let mut out = vec![];

    for i in 0..BITS {
        out.push(tseytin_xor(x[i].clone(), y[i].clone(), clauses, symbols))
    }

    // TODO: compare generated expression to using MUX
    let mask;

    if min {
        // mask is 1 if x > y
        mask = inequality_boolean(x.clone(), y.clone(), true, clauses, symbols);
    } else {
        // flip the args if getting maximum x < y -> 1
        mask = inequality_boolean(y.clone(), x.clone(), true, clauses, symbols);
    }

    for i in 0..BITS {
        out[i] = tseytin_and(&vec![out[i].clone(), mask.clone()], clauses, symbols);
    }

    for i in 0..BITS {
        out[i] = tseytin_xor(x[i].clone(), out[i].clone(), clauses, symbols);
    }

    out
}

/// Converts max of CnfInts to a single CnfInt
///
/// ```text
/// Max(CnfInt(a), CnfInt(b), ...) ~> CnfInt(c)
///
/// ```
#[register_rule(("CNF", 4100))]
fn cnf_int_max(expr: &Expr, symbols: &SymbolTable) -> ApplicationResult {
    let Expr::Max(_, exprs) = expr else {
        return Err(RuleNotApplicable);
    };

    let Expr::AbstractLiteral(_, Matrix(exprs_list, _)) = exprs.as_ref() else {
        return Err(RuleNotApplicable);
    };

    let mut exprs_bits = validate_cnf_int_operands(exprs_list.clone())?;

    let mut new_symbols = symbols.clone();
    let mut values;
    let mut new_clauses = vec![];

    while exprs_bits.len() > 1 {
        let mut next = Vec::with_capacity((exprs_bits.len() + 1) / 2);
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
        Expr::CnfInt(Metadata::new(), Moo::new(into_matrix_expr!(result))),
        new_clauses,
        new_symbols,
    ))
}

/// Converts Abs of a CnfInt to a CnfInt
///
/// ```text
/// |CnfInt(a)| ~> CnfInt(b)
///
/// ```
#[register_rule(("CNF", 4100))]
fn cnf_int_abs(expr: &Expr, symbols: &SymbolTable) -> ApplicationResult {
    let Expr::Abs(_, expr) = expr else {
        return Err(RuleNotApplicable);
    };

    let binding = validate_cnf_int_operands(vec![expr.as_ref().clone()])?;
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
        Expr::CnfInt(Metadata::new(), Moo::new(into_matrix_expr!(result))),
        new_clauses,
        new_symbols,
    ))
}

/// Converts SafeDiv of CnfInts to a single CnfInt
///
/// ```text
/// SafeDiv(CnfInt(a), CnfInt(b)) ~> CnfInt(c)
///
/// ```
#[register_rule(("CNF", 4100))]
fn cnf_int_safediv(expr: &Expr, symbols: &SymbolTable) -> ApplicationResult {
    // Using "Restoring division" algorithm
    // https://en.wikipedia.org/wiki/Division_algorithm#Restoring_division
    let Expr::SafeDiv(_, numer, denom) = expr else {
        return Err(RuleNotApplicable);
    };

    let binding = validate_cnf_int_operands(vec![numer.as_ref().clone(), denom.as_ref().clone()])?;
    let [numer_bits, denom_bits] = binding.as_slice() else {
        return Err(RuleNotApplicable);
    };

    // TODO: Separate into division/mod function
    // TODO: Support negatives

    let mut new_symbols = symbols.clone();
    let mut new_clauses = vec![];
    let mut quotient = vec![false.into(); BITS];

    let mut r = numer_bits.clone();
    r.extend(std::iter::repeat(r[BITS - 1].clone()).take(BITS));
    let mut d = std::iter::repeat(false.into()).take(BITS).collect_vec();
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
        Expr::CnfInt(Metadata::new(), Moo::new(into_matrix_expr!(quotient))),
        new_clauses,
        new_symbols,
    ))
}

/*
/// Converts SafeMod of CnfInts to a single CnfInt
///
/// ```text
/// SafeMod(CnfInt(a), CnfInt(b)) ~> CnfInt(c)
///
/// ```
#[register_rule(("CNF", 4100))]
fn cnf_int_safemod(expr: &Expr, _: &SymbolTable) -> ApplicationResult {}

/// Converts SafePow of CnfInts to a single CnfInt
///
/// ```text
/// SafePow(CnfInt(a), CnfInt(b)) ~> CnfInt(c)
///
/// ```
#[register_rule(("CNF", 4100))]
fn cnf_int_safepow(expr: &Expr, _: &SymbolTable) -> ApplicationResult {
    // use 'Exponentiation by squaring'
}
*/
