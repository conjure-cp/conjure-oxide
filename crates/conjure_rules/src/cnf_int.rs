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

    let (output, new_symbols, new_tops) =
        inequality_boolean(lhs_bits.clone(), rhs_bits.clone(), strict, symbols);

    Ok(Reduction::new(output, new_tops, new_symbols))
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
fn cnf_int_eq(expr: &Expr, _: &SymbolTable) -> ApplicationResult {
    let Expr::Eq(_, lhs, rhs) = expr else {
        return Err(RuleNotApplicable);
    };

    let binding = validate_cnf_int_operands(vec![unbox(lhs), unbox(rhs)])?;
    let [lhs_bits, rhs_bits] = binding.as_slice() else {
        return Err(RuleNotApplicable);
    };

    let output = lhs_bits
        .iter()
        .zip(rhs_bits.iter())
        .map(|(x_i, y_i)| {
            Expr::Iff(
                Metadata::new(),
                Box::new(x_i.clone()),
                Box::new(y_i.clone()),
            )
        })
        .collect();

    Ok(Reduction::pure(Expr::And(
        Metadata::new(),
        Box::new(into_matrix_expr!(output)),
    )))
}

/// Converts a != expression between two CnfInts to a disjunction of boolean expressions
///
/// ```text
/// CnfInt(a) != CnfInt(b) ~> Or(...)
///
/// ```
#[register_rule(("CNF", 4100))]
fn cnf_int_neq(expr: &Expr, _: &SymbolTable) -> ApplicationResult {
    let Expr::Neq(_, lhs, rhs) = expr else {
        return Err(RuleNotApplicable);
    };

    let binding = validate_cnf_int_operands(vec![unbox(lhs), unbox(rhs)])?;
    let [lhs_bits, rhs_bits] = binding.as_slice() else {
        return Err(RuleNotApplicable);
    };

    let output = lhs_bits
        .iter()
        .zip(rhs_bits.iter())
        .map(|(x_i, y_i)| {
            Expr::Not(
                Metadata::new(),
                Box::new(Expr::Iff(
                    Metadata::new(),
                    Box::new(x_i.clone()),
                    Box::new(y_i.clone()),
                )),
            )
        })
        .collect();

    Ok(Reduction::pure(Expr::Or(
        Metadata::new(),
        Box::new(into_matrix_expr!(output)),
    )))
}

// Creates a boolean expression for > or >=
// a > b or a >= b
// This can also be used for < and <= by reversing the order of the inputs
// Returns result, new symbol table, new top level constraints
fn inequality_boolean(
    a: Vec<Expr>,
    b: Vec<Expr>,
    strict: bool,
    symbols: &SymbolTable,
) -> (Expr, SymbolTable, Vec<Expr>) {
    let mut output;

    let mut new_tops = vec![];
    let mut temp;
    let mut new_symbols;
    let mut notb;

    if strict {
        (output, new_symbols, temp) = tseytin_imply(a[0].clone(), b[0].clone(), symbols);
        new_tops.extend(temp);
    } else {
        (notb, new_symbols, temp) = tseytin_not(b[0].clone(), symbols);
        new_tops.extend(temp);

        (output, new_symbols, temp) = tseytin_and(&vec![a[0].clone(), notb], &new_symbols);
        new_tops.extend(temp);
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
        (notb, new_symbols, temp) = tseytin_not(b[n].clone(), &new_symbols);
        new_tops.extend(temp);

        (lhs, new_symbols, temp) = tseytin_and(&vec![a[n].clone(), notb.clone()], &new_symbols);
        new_tops.extend(temp);

        (iff, new_symbols, temp) = tseytin_iff(a[n].clone(), b[n].clone(), &new_symbols);
        new_tops.extend(temp);

        (rhs, new_symbols, temp) = tseytin_and(&vec![iff.clone(), output.clone()], &new_symbols);
        new_tops.extend(temp);

        (output, new_symbols, temp) = tseytin_or(&vec![lhs.clone(), rhs.clone()], &new_symbols);
        new_tops.extend(temp);
    }

    // final bool is the sign bit and should be handled inversely
    // a_n = &a[7];
    // b_n = &b[7];
    // output = essence_expr!(r"((-&a_n /\ &b_n) \/ (((&a_n /\ &b_n) \/ (-&a_n /\ -&b_n)) /\ &output))");
    let nota;
    (nota, new_symbols, temp) = tseytin_not(a[7].clone(), &new_symbols);
    new_tops.extend(temp);

    (lhs, new_symbols, temp) = tseytin_and(&vec![nota, b[7].clone()], &new_symbols);
    new_tops.extend(temp);

    (iff, new_symbols, temp) = tseytin_iff(a[7].clone(), b[7].clone(), &new_symbols);
    new_tops.extend(temp);

    (rhs, new_symbols, temp) = tseytin_and(&vec![iff.clone(), output.clone()], &new_symbols);
    new_tops.extend(temp);

    (output, new_symbols, temp) = tseytin_or(&vec![lhs.clone(), rhs.clone()], &new_symbols);
    new_tops.extend(temp);

    (output, new_symbols, new_tops)
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
    let mut temp;
    let mut new_tops = vec![];

    while exprs_bits.len() > 1 {
        let mut next = Vec::with_capacity((exprs_bits.len() + 1) / 2);
        let mut iter = exprs_bits.into_iter();

        while let Some(a) = iter.next() {
            if let Some(b) = iter.next() {
                (values, new_symbols, temp) = tseytin_int_adder(a, b, &new_symbols);
                new_tops.extend(temp);
                next.push(values);
            } else {
                next.push(a);
            }
        }

        exprs_bits = next;
    }

    let result = exprs_bits.pop().unwrap();

    Ok(Reduction::new(
        Expr::CnfInt(Metadata::new(), Box::new(into_matrix_expr!(result))),
        new_tops,
        new_symbols,
    ))
}

fn cnf_int_adder(x: Vec<Expr>, y: Vec<Expr>) -> Vec<Expr> {
    // let mut x_n = x[0].clone();
    // let mut y_n = y[0].clone();

    // let mut output = vec![essence_expr!(r"(-&x_n /\ &y_n) \/ (-&y_n /\ &x_n)")];
    let mut output = vec![Expr::Not(
        Metadata::new(),
        Box::new(Expr::Iff(
            Metadata::new(),
            Box::new(x[0].clone()),
            Box::new(y[0].clone()),
        )),
    )];

    //let mut carry = essence_expr!(r"&x_n /\ &y_n");
    let mut carry = Expr::And(
        Metadata::new(),
        Box::new(matrix_expr![x[0].clone(), y[0].clone()]),
    );

    for i in 1..8 {
        // x_n = x[i].clone();
        // y_n = y[i].clone();

        // output.push(essence_expr!(
        //     r"(&x_n /\ &y_n) \/ (&carry /\ ((-&x_n /\ &y_n) \/ (-&y_n /\ &x_n)))"
        // ));

        output.push(Expr::Not(
            Metadata::new(),
            Box::new(Expr::Iff(
                Metadata::new(),
                Box::new(carry.clone()),
                Box::new(Expr::Not(
                    Metadata::new(),
                    Box::new(Expr::Iff(
                        Metadata::new(),
                        Box::new(x[i].clone()),
                        Box::new(y[i].clone()),
                    )),
                )),
            )),
        ));

        // carry = essence_expr!(
        //     r"((-&carry /\ ((-&x_n /\ &y_n) \/ (-&y_n /\ &x_n))) \/ (-((-&x_n /\ &y_n) \/ (-&y_n /\ &x_n)) /\ &carry))"
        // )
        carry = Expr::Or(
            Metadata::new(),
            Box::new(matrix_expr![
                Expr::And(
                    Metadata::new(),
                    Box::new(matrix_expr![x[i].clone(), y[i].clone()])
                ),
                Expr::And(
                    Metadata::new(),
                    Box::new(matrix_expr![
                        carry.clone(),
                        Expr::Not(
                            Metadata::new(),
                            Box::new(Expr::Iff(
                                Metadata::new(),
                                Box::new(x[i].clone()),
                                Box::new(y[i].clone())
                            ))
                        )
                    ])
                )
            ]),
        )
    }

    output
}

// Returns result, new symbol table, new top level constraints
fn tseytin_int_adder(
    x: Vec<Expr>,
    y: Vec<Expr>,
    symbols: &SymbolTable,
) -> (Vec<Expr>, SymbolTable, Vec<Expr>) {
    let (mut result, mut new_symbols, mut new_tops) =
        tseytin_xor(x[0].clone(), y[0].clone(), symbols);
    let mut output = vec![result];

    let mut carry;
    let mut temp;
    (carry, new_symbols, temp) = tseytin_and(&vec![x[0].clone(), y[0].clone()], &new_symbols);
    new_tops.extend(temp);

    for i in 1..8 {
        (result, carry, new_symbols, temp) =
            tseytin_full_adder(x[i].clone(), y[i].clone(), carry.clone(), &new_symbols);
        output.push(result);
        new_tops.extend(temp);
    }

    (output, new_symbols, new_tops)
}

// Returns: result, carry, new symbol table, new top level constraints
fn tseytin_full_adder(
    a: Expr,
    b: Expr,
    carry: Expr,
    symbols: &SymbolTable,
) -> (Expr, Expr, SymbolTable, Vec<Expr>) {
    let mut temp;
    let axorb;
    let result;
    let mut new_tops = vec![];
    let aandb;
    let carryandaxorb;
    let carryout;
    let mut new_symbols;

    (axorb, new_symbols, temp) = tseytin_xor(a.clone(), b.clone(), symbols);
    new_tops.extend(temp);
    (result, new_symbols, temp) = tseytin_xor(axorb.clone(), carry.clone(), &new_symbols);
    new_tops.extend(temp);
    (aandb, new_symbols, temp) = tseytin_and(&vec![a, b], &new_symbols);
    new_tops.extend(temp);
    (carryandaxorb, new_symbols, temp) = tseytin_and(&vec![carry, axorb], &new_symbols);
    new_tops.extend(temp);
    (carryout, new_symbols, temp) = tseytin_or(&vec![aandb, carryandaxorb], &new_symbols);
    new_tops.extend(temp);

    (result, carryout, new_symbols, new_tops)
}

/*
/// Converts product of CnfInts to a single CnfInt
///
/// ```text
/// Product(CnfInt(a), CnfInt(b), ...) ~> CnfInt(c)
///
/// ```
#[register_rule(("CNF", 4100))]
fn cnf_int_product(expr: &Expr, _: &SymbolTable) -> ApplicationResult {
    // create multiple products
}

/// Converts negation of a CnfInt to a CnfInt
///
/// ```text
/// -CnfInt(a) ~> CnfInt(b)
///
/// ```
#[register_rule(("CNF", 4100))]
fn cnf_int_neg(expr: &Expr, _: &SymbolTable) -> ApplicationResult {
    // invert then add 1
}

/// Converts min of CnfInts to a single CnfInt
///
/// ```text
/// Min(CnfInt(a), CnfInt(b), ...) ~> CnfInt(c)
///
/// ```
#[register_rule(("CNF", 4100))]
fn cnf_int_min(expr: &Expr, _: &SymbolTable) -> ApplicationResult {
    // use conditionals
}

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
