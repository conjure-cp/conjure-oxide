use conjure_core::ast::Expression as Expr;
use conjure_core::ast::SymbolTable;
use conjure_core::rule_engine::{
    register_rule, ApplicationError, ApplicationError::RuleNotApplicable, ApplicationResult,
    Reduction,
};

use conjure_core::ast::AbstractLiteral::Matrix;
use conjure_core::ast::{Atom, Domain, Literal, Range};
use conjure_core::metadata::Metadata;
use conjure_core::{into_matrix_expr, matrix_expr};

use conjure_essence_macros::essence_expr;

use crate::cnf::tseytin_and;
use crate::cnf::tseytin_iff;
use crate::cnf::tseytin_imply;
use crate::cnf::tseytin_not;
use crate::cnf::tseytin_or;
use crate::cnf::tseytin_xor;

#[register_rule(("CNF", 9500))]
fn integer_decision_representation(expr: &Expr, symbols: &SymbolTable) -> ApplicationResult {
    // thing we are representing must be a reference
    let Expr::Atomic(_, Atom::Reference(name)) = expr else {
        return Err(RuleNotApplicable);
    };

    // thing we are representing must be a variable
    symbols
        .lookup(name)
        .ok_or(RuleNotApplicable)?
        .as_var()
        .ok_or(RuleNotApplicable)?;

    // thing we are representing must be an integer
    let Domain::IntDomain(ranges) = &symbols.resolve_domain(name).unwrap() else {
        return Err(RuleNotApplicable);
    };

    let mut symbols = symbols.clone();

    let repr_exists = symbols.get_representation(name, &["int_to_atom"]).is_some();

    let representation = symbols
        .get_or_add_representation(name, &["int_to_atom"])
        .ok_or(RuleNotApplicable)?;

    let bits = representation[0]
        .clone()
        .expression_down(&symbols)?
        .into_iter()
        .map(|(_, expr)| expr.clone())
        .collect();

    let cnf_int = Expr::CnfInt(Metadata::new(), Box::new(into_matrix_expr!(bits)));

    if !repr_exists {
        // add domain ranges as constraints if this is the first time the representation is added
        Ok(Reduction::new(
            cnf_int.clone(),
            vec![int_domain_to_expr(cnf_int.clone(), ranges)], // contains domain rules
            symbols,
        ))
    } else {
        Ok(Reduction::with_symbols(cnf_int.clone(), symbols))
    }
}

#[register_rule(("CNF", 9500))]
fn literal_cnf_int(expr: &Expr, _: &SymbolTable) -> ApplicationResult {
    let Expr::Atomic(_, Atom::Literal(Literal::Int(mut value))) = expr else {
        return Err(RuleNotApplicable);
    };

    let mut binary_encoding = vec![];

    // CHANGE TO 32
    for _ in 0..8 {
        binary_encoding.push(Expr::Atomic(
            Metadata::new(),
            Atom::Literal(Literal::Bool((value & 1) != 0)),
        ));
        value >>= 1;
    }

    Ok(Reduction::pure(Expr::CnfInt(
        Metadata::new(),
        Box::new(into_matrix_expr!(binary_encoding)),
    )))
}

// This function takes a target expression and a vector of ranges and creates an expression representing the ranges with the target expression as the subject
//
// E.g. x : int(4), int(10..20), int(30..) ~> Or(x=4, 10<=x<=20, x>=30)
fn int_domain_to_expr(subject: Expr, ranges: &Vec<Range<i32>>) -> Expr {
    let mut output = vec![];

    let value = Box::new(subject);

    for range in ranges {
        match range {
            Range::Single(x) => output.push(essence_expr!(&value = &x)),
            Range::Bounded(x, y) => output.push(essence_expr!("&value >= &x /\\ &value <= &y")),
            Range::UnboundedR(x) => output.push(essence_expr!(&value >= &x)),
            Range::UnboundedL(x) => output.push(essence_expr!(&value <= &x)),
        }
    }

    Expr::Or(Metadata::new(), Box::new(into_matrix_expr!(output)))
}

/// Converts an inequality expression between two CnfInts to a conjunction of boolean expressions
///
/// ```text
/// CnfInt(a) </>/<=/>= CnfInt(b) ~> And(...)
///
/// ```
#[register_rule(("CNF", 4100))]
fn cnf_int_ineq(expr: &Expr, symbols: &SymbolTable) -> ApplicationResult {
    let (lhs, rhs, strict) = match expr {
        Expr::Lt(_, x, y) => (y, x, false),
        Expr::Gt(_, x, y) => (x, y, false),
        Expr::Leq(_, x, y) => (y, x, true),
        Expr::Geq(_, x, y) => (x, y, true),
        _ => return Err(RuleNotApplicable),
    };

    let binding = validate_cnf_int_operands(vec![unbox(lhs), unbox(rhs)])?;
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

fn unbox(expr: &Box<Expr>) -> Expr {
    (**expr).clone()
}

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

/// Converts a = expression between two CnfInts to a conjunction of boolean expressions
///
/// ```text
/// CnfInt(a) = CnfInt(b) ~> And(...)
///
/// ```
#[register_rule(("CNF", 9100))]
fn cnf_int_eq(expr: &Expr, symbols: &SymbolTable) -> ApplicationResult {
    let Expr::Eq(_, lhs, rhs) = expr else {
        return Err(RuleNotApplicable);
    };

    let binding = validate_cnf_int_operands(vec![unbox(lhs), unbox(rhs)])?;
    let [lhs_bits, rhs_bits] = binding.as_slice() else {
        return Err(RuleNotApplicable);
    };

    let mut output = true.into();
    let mut new_symbols = symbols.clone();
    let mut new_clauses = vec![];
    let mut comparison;

    for i in 0..8 {
        // BITS
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

/// Converts a != expression between two CnfInts to a disjunction of boolean expressions
///
/// ```text
/// CnfInt(a) != CnfInt(b) ~> Or(...)
///
/// ```
#[register_rule(("CNF", 4100))]
fn cnf_int_neq(expr: &Expr, symbols: &SymbolTable) -> ApplicationResult {
    let Expr::Neq(_, lhs, rhs) = expr else {
        return Err(RuleNotApplicable);
    };

    let binding = validate_cnf_int_operands(vec![unbox(lhs), unbox(rhs)])?;
    let [lhs_bits, rhs_bits] = binding.as_slice() else {
        return Err(RuleNotApplicable);
    };

    let mut output = false.into();
    let mut new_symbols = symbols.clone();
    let mut new_clauses = vec![];
    let mut comparison;

    for i in 0..8 {
        // BITS
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
        output = tseytin_imply(a[0].clone(), b[0].clone(), clauses, symbols);
    } else {
        notb = tseytin_not(b[0].clone(), clauses, symbols);
        output = tseytin_and(&vec![a[0].clone(), notb], clauses, symbols);
    }

    let mut lhs;
    let mut rhs;
    let mut iff;
    // at the moment this causes a stack overflow
    // CHANGE TO 32
    for n in 1..7 {
        // a_n = &a[n];
        // b_n = &b[n];
        // Macro expression is commented out at the moment because it causes the program to hang for some reason
        // output = essence_expr!(r"((&a_n /\ -&b_n) \/ (((&a_n /\ &b_n) \/ (-&a_n /\ -&b_n)) /\ &output))");
        notb = tseytin_not(b[n].clone(), clauses, symbols);
        lhs = tseytin_and(&vec![a[n].clone(), notb.clone()], clauses, symbols);
        iff = tseytin_iff(a[n].clone(), b[n].clone(), clauses, symbols);
        rhs = tseytin_and(&vec![iff.clone(), output.clone()], clauses, symbols);
        output = tseytin_or(&vec![lhs.clone(), rhs.clone()], clauses, symbols);
    }

    // final bool is the sign bit and should be handled inversely
    // a_n = &a[7];
    // b_n = &b[7];
    // output = essence_expr!(r"((-&a_n /\ &b_n) \/ (((&a_n /\ &b_n) \/ (-&a_n /\ -&b_n)) /\ &output))");
    let nota;
    nota = tseytin_not(a[7].clone(), clauses, symbols);
    lhs = tseytin_and(&vec![nota, b[7].clone()], clauses, symbols);
    iff = tseytin_iff(a[7].clone(), b[7].clone(), clauses, symbols);
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
                values = tseytin_int_adder(&a, &b, 8, &mut new_clauses, &mut new_symbols);
                next.push(values);
            } else {
                next.push(a);
            }
        }

        exprs_bits = next;
    }

    let result = exprs_bits.pop().unwrap();

    Ok(Reduction::cnf(
        Expr::CnfInt(Metadata::new(), Box::new(into_matrix_expr!(result))),
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
    let mut result = tseytin_xor(x[0].clone(), y[0].clone(), clauses, symbols);

    let mut output = vec![result];
    let mut carry;

    carry = tseytin_and(&vec![x[0].clone(), y[0].clone()], clauses, symbols);
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
    clauses: &mut Vec<Expr>,
    symbols: &mut SymbolTable,
) -> Vec<Expr> {
    let mut result = vec![];
    let mut product = expr[exponent].clone();

    for i in 0..exponent {
        result.push(expr[i].clone());
    }

    result.push(tseytin_not(expr[exponent].clone(), clauses, symbols));

    for i in (exponent + 1)..8 {
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
    clauses: &mut Vec<Expr>,
    symbols: &mut SymbolTable,
) -> Vec<Expr> {
    let mut x = x.clone();
    let mut y = y.clone();

    let bits = 8; // TODO: remove

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
        println!("{}", n);
        // y << 1
        for i in (1..bits * 2).rev() {
            y[i] = y[i - 1].clone();
        }
        y[0] = false.into();

        sum = tseytin_int_adder(&s, &y, 16, clauses, symbols);
        not_x_n = tseytin_not(x[n].clone(), clauses, symbols);

        for i in 0..16 {
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
    let Expr::Product(_, exprs_list) = expr else {
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
                values = cnf_shift_add_multiply(&a, &b, &mut new_clauses, &mut new_symbols);
                next.push(values);
            } else {
                next.push(a);
            }
        }

        exprs_bits = next;
    }

    let result = exprs_bits.pop().unwrap();

    Ok(Reduction::cnf(
        Expr::CnfInt(Metadata::new(), Box::new(into_matrix_expr!(result))),
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

    let binding = validate_cnf_int_operands(vec![unbox(expr)])?;
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
    result = tseytin_add_two_power(&result, 0, &mut new_clauses, &mut new_symbols);

    Ok(Reduction::cnf(
        Expr::CnfInt(Metadata::new(), Box::new(into_matrix_expr!(result))),
        new_clauses,
        new_symbols,
    ))
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
                values = tseytin_binary_min(&a, &b, &mut new_clauses, &mut new_symbols);
                next.push(values);
            } else {
                next.push(a);
            }
        }

        exprs_bits = next;
    }

    let result = exprs_bits.pop().unwrap();

    Ok(Reduction::cnf(
        Expr::CnfInt(Metadata::new(), Box::new(into_matrix_expr!(result))),
        new_clauses,
        new_symbols,
    ))
}

fn tseytin_binary_min(
    x: &Vec<Expr>,
    y: &Vec<Expr>,
    clauses: &mut Vec<Expr>,
    symbols: &mut SymbolTable,
) -> Vec<Expr> {
    vec![false.into()]
}

/*
/// Converts max of CnfInts to a single CnfInt
///
/// ```text
/// Max(CnfInt(a), CnfInt(b), ...) ~> CnfInt(c)
///
/// ```
#[register_rule(("CNF", 4100))]
fn cnf_int_max(expr: &Expr, _: &SymbolTable) -> ApplicationResult {
    // use conditionals
}

/// Converts Abs of a CnfInt to a CnfInt
///
/// ```text
/// |CnfInt(a)| ~> CnfInt(b)
///
/// ```
#[register_rule(("CNF", 4100))]
fn cnf_int_neg(expr: &Expr, _: &SymbolTable) -> ApplicationResult {
    // negate if sign bit is 1
}

/// Converts SafeDiv of CnfInts to a single CnfInt
///
/// ```text
/// SafeDiv(CnfInt(a), CnfInt(b)) ~> CnfInt(c)
///
/// ```
#[register_rule(("CNF", 4100))]
fn cnf_int_safediv(expr: &Expr, _: &SymbolTable) -> ApplicationResult {
    // binary div
}

/// Converts Minus of CnfInts to a single CnfInt
///
/// ```text
/// Minus(CnfInt(a), CnfInt(b)) ~> CnfInt(c)
///
/// ```
#[register_rule(("CNF", 4100))]
fn cnf_int_minus(expr: &Expr, _: &SymbolTable) -> ApplicationResult {
    // minus circuit (support 2s complement)
}

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
