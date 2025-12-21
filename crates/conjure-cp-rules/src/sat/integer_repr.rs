use conjure_cp::ast::{Expression as Expr, GroundDomain};
use conjure_cp::ast::{SATIntEncoding, SymbolTable};
use conjure_cp::rule_engine::{
    ApplicationError, ApplicationError::RuleNotApplicable, ApplicationResult, Reduction,
    register_rule,
};

use conjure_cp::ast::Metadata;
use conjure_cp::ast::{Atom, Literal, Moo, Range};
use conjure_cp::into_matrix_expr;

use conjure_cp::{bug, essence_expr};

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
            _ => bug!("Unbounded domains not supported for SAT"),
        }
    }

    Expr::Or(Metadata::new(), Moo::new(into_matrix_expr!(output)))
}

/// This function confirms that all of the input expressions are log SATInts, and returns vectors for each input of their bits
/// This function also extends all vectors such that they have the same lengths
/// The vector lengths is either `n` for bit_count = Some(n), otherwise the length of the longest operand
pub fn validate_log_int_operands(
    exprs: Vec<Expr>,
    bit_count: Option<u32>,
) -> Result<Vec<Vec<Expr>>, ApplicationError> {
    // TODO: In the future it may be possible to optimize operations between integers with different bit sizes
    // Collect inner bit vectors from each SATInt
    let mut out: Vec<Vec<Expr>> = exprs
        .into_iter()
        .map(|expr| {
            let Expr::SATInt(_, SATIntEncoding::Log, inner, _) = expr else {
                return Err(RuleNotApplicable);
            };
            let Some(bits) = inner.as_ref().clone().unwrap_list() else {
                return Err(RuleNotApplicable);
            };
            Ok(bits)
        })
        .collect::<Result<_, _>>()?;

    // Determine target length
    let max_len = bit_count
        .map(|b| b as usize)
        .unwrap_or_else(|| out.iter().map(|v| v.len()).max().unwrap_or(0));

    // Extend or crop each vector
    for v in &mut out {
        if v.len() < max_len {
            // pad with the last element
            if let Some(last) = v.last().cloned() {
                v.resize(max_len, last);
            }
        } else if v.len() > max_len {
            // crop extra elements
            v.truncate(max_len);
        }
    }

    Ok(out)
}

/// Converts an integer decision variable to SATInt form, creating a new representation of boolean variables if
/// one does not yet exist
///
/// ```text
///  x
///  ~~>
///  SATInt([x#00, x#01, ...])
///
///  new variables:
///  find x#00: bool
///  find x#01: bool
///  ...
///
/// ```
#[register_rule(("SAT", 9500))]
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

    let new_name = &name.name().to_owned();

    let repr_exists = symbols
        .get_representation(new_name, &["sat_log_int"])
        .is_some();

    let representation = symbols
        .get_or_add_representation(new_name, &["sat_log_int"])
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

    if !repr_exists {
        // add domain ranges as constraints if this is the first time the representation is added
        Ok(Reduction::new(
            cnf_int.clone(),
            vec![int_domain_to_expr(cnf_int, ranges)], // contains domain rules
            symbols,
        ))
    } else {
        Ok(Reduction::pure(cnf_int))
    }
}

/// Converts an integer literal to SATInt form
///
/// ```text
///  3
///  ~~>
///  SATInt([true,true,false,false,false,false,false,false;int(1..)])
///
/// ```
#[register_rule(("SAT", 9500))]
fn literal_cnf_int(expr: &Expr, _: &SymbolTable) -> ApplicationResult {
    let value = {
        if let Expr::Atomic(_, Atom::Literal(Literal::Int(v))) = expr {
            *v
        } else {
            return Err(RuleNotApplicable);
        }
    };

    //TODO: add support for negatives
    //TODO: Adding constant optimization to all int operations should hopefully make this rule redundant

    let mut binary_encoding = vec![];

    let bit_count = bit_magnitude(value);

    let mut value_mut = value as u32;

    for _ in 0..bit_count {
        binary_encoding.push(Expr::Atomic(
            Metadata::new(),
            Atom::Literal(Literal::Bool((value_mut & 1) != 0)),
        ));
        value_mut >>= 1;
    }

    Ok(Reduction::pure(Expr::SATInt(
        Metadata::new(),
        SATIntEncoding::Log,
        Moo::new(into_matrix_expr!(binary_encoding)),
        (value, value),
    )))
}

/// Determine the number of bits required to encode an i32 in 2s complement
pub fn bit_magnitude(x: i32) -> usize {
    if x >= 0 {
        // positive: bits = highest set bit + 1 sign bit
        (1 + (32 - x.leading_zeros())).try_into().unwrap()
    } else {
        // negative: bits = highest set bit in magnitude
        (33 - (!x).leading_zeros()).try_into().unwrap()
    }
}

/// Given two vectors of expressions, extend the shorter one by repeating its last element until both are the same length
pub fn match_bits_length(a: Vec<Expr>, b: Vec<Expr>) -> (Vec<Expr>, Vec<Expr>) {
    let len_a = a.len();
    let len_b = b.len();

    if len_a < len_b {
        let last_a = a.last().cloned().unwrap();
        let mut a_extended = a;
        a_extended.resize(len_b, last_a);
        (a_extended, b)
    } else if len_b < len_a {
        let last_b = b.last().cloned().unwrap();
        let mut b_extended = b;
        b_extended.resize(len_a, last_b);
        (a, b_extended)
    } else {
        (a, b)
    }
}
