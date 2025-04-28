use conjure_core::ast::Expression as Expr;
use conjure_core::ast::SymbolTable;
use conjure_core::rule_engine::{
    register_rule, ApplicationError::RuleNotApplicable, ApplicationResult, Reduction,
};

use conjure_core::ast::{Atom, Domain, Literal, Range};
use conjure_core::metadata::Metadata;
use conjure_core::{into_matrix_expr, matrix_expr};

use std::mem;

#[register_rule(("CNF", 8000))]
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

#[register_rule(("CNF", 4000))]
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
            Range::Single(x) => output.push(Expr::Eq(
                Metadata::new(),
                value.clone(),
                Box::new(Expr::Atomic(
                    Metadata::new(),
                    Atom::Literal(Literal::Int(*x)),
                )),
            )),
            Range::Bounded(x, y) => output.push(Expr::And(
                Metadata::new(),
                Box::new(matrix_expr![
                    Expr::Geq(
                        Metadata::new(),
                        value.clone(),
                        Box::new(Expr::Atomic(
                            Metadata::new(),
                            Atom::Literal(Literal::Int(*x))
                        )),
                    ),
                    Expr::Leq(
                        Metadata::new(),
                        value.clone(),
                        Box::new(Expr::Atomic(
                            Metadata::new(),
                            Atom::Literal(Literal::Int(*y))
                        )),
                    )
                ]),
            )),
            Range::UnboundedR(x) => output.push(Expr::Geq(
                Metadata::new(),
                value.clone(),
                Box::new(Expr::Atomic(
                    Metadata::new(),
                    Atom::Literal(Literal::Int(*x)),
                )),
            )),
            Range::UnboundedL(x) => output.push(Expr::Leq(
                Metadata::new(),
                value.clone(),
                Box::new(Expr::Atomic(
                    Metadata::new(),
                    Atom::Literal(Literal::Int(*x)),
                )),
            )),
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
fn cnf_int_ineq(expr: &Expr, _: &SymbolTable) -> ApplicationResult {
    let (lhs, rhs, inclusive) = match expr {
        Expr::Lt(_, x, y) => (y, x, false),
        Expr::Gt(_, x, y) => (x, y, false),
        Expr::Leq(_, x, y) => (y, x, true),
        Expr::Geq(_, x, y) => (x, y, true),
        _ => return Err(RuleNotApplicable),
    };

    let Expr::CnfInt(_, lhs) = lhs.as_ref() else {
        return Err(RuleNotApplicable);
    };

    let Expr::CnfInt(_, rhs) = rhs.as_ref() else {
        return Err(RuleNotApplicable);
    };

    let Some(lhs_bits) = lhs.as_ref().clone().unwrap_list() else {
        return Err(RuleNotApplicable);
    };

    let Some(rhs_bits) = rhs.as_ref().clone().unwrap_list() else {
        return Err(RuleNotApplicable);
    };

    let output = inequality_boolean(lhs_bits, rhs_bits, inclusive);

    Ok(Reduction::pure(output))
}

/// Converts a = expression between two CnfInts to a conjunction of boolean expressions
///
/// ```text
/// CnfInt(a) = CnfInt(b) ~> And(...)
///
/// ```
#[register_rule(("CNF", 4100))]
fn cnf_int_eq(expr: &Expr, _: &SymbolTable) -> ApplicationResult {
    let Expr::Eq(_, x, y) = expr else {
        return Err(RuleNotApplicable);
    };

    let Expr::CnfInt(_, x) = x.as_ref() else {
        return Err(RuleNotApplicable);
    };

    let Expr::CnfInt(_, y) = y.as_ref() else {
        return Err(RuleNotApplicable);
    };

    let Some(x_bits) = x.as_ref().clone().unwrap_list() else {
        return Err(RuleNotApplicable);
    };

    let Some(y_bits) = y.as_ref().clone().unwrap_list() else {
        return Err(RuleNotApplicable);
    };

    let output = x_bits
        .iter()
        .zip(y_bits.iter())
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
    let Expr::Neq(_, x, y) = expr else {
        return Err(RuleNotApplicable);
    };

    let Expr::CnfInt(_, x) = x.as_ref() else {
        return Err(RuleNotApplicable);
    };

    let Expr::CnfInt(_, y) = y.as_ref() else {
        return Err(RuleNotApplicable);
    };

    let Some(x_bits) = x.as_ref().clone().unwrap_list() else {
        return Err(RuleNotApplicable);
    };

    let Some(y_bits) = y.as_ref().clone().unwrap_list() else {
        return Err(RuleNotApplicable);
    };

    let output = x_bits
        .iter()
        .zip(y_bits.iter())
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
fn inequality_boolean(a: Vec<Expr>, b: Vec<Expr>, inclusive: bool) -> Expr {
    let mut output;

    if inclusive {
        output = Expr::Imply(
            Metadata::new(),
            Box::new(b[0].clone()),
            Box::new(a[0].clone()),
        );
    } else {
        output = Expr::And(
            Metadata::new(),
            Box::new(matrix_expr![
                a[0].clone(),
                Expr::Not(Metadata::new(), Box::new(b[0].clone()))
            ]),
        );
    }

    // at the moment this causes a stack overflow
    // CHANGE TO 32
    for n in 1..8 {
        println!("{}\n", output);
        output = Expr::Or(
            Metadata::new(),
            Box::new(matrix_expr![
                Expr::And(
                    Metadata::new(),
                    Box::new(matrix_expr![
                        a[n].clone(),
                        Expr::Not(Metadata::new(), Box::new(b[n].clone()))
                    ]),
                ),
                Expr::And(
                    Metadata::new(),
                    Box::new(matrix_expr![
                        Expr::Iff(
                            Metadata::new(),
                            Box::new(a[n].clone()),
                            Box::new(b[n].clone())
                        ),
                        output
                    ])
                )
            ]),
        );
    }
    output
}

/* /// Converts sum of CnfInts to a single CnfInt
///
/// ```text
/// Sum(CnfInt(a), CnfInt(b), ...) ~> CnfInt(c)
///
/// ```
#[register_rule(("CNF", 4100))]
fn cnf_int_sum(expr: &Expr, _: &SymbolTable) -> ApplicationResult {
    // create multiple adders
}

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
} */
