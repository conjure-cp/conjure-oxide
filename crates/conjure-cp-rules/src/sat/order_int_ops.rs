use conjure_cp::ast::{Atom, Expression as Expr, Literal};
use conjure_cp::ast::{SATIntEncoding, SymbolTable};
use conjure_cp::rule_engine::ApplicationError;
use conjure_cp::rule_engine::{
    ApplicationError::RuleNotApplicable, ApplicationResult, Reduction, register_rule,
};

use crate::sat::boolean::{tseytin_and, tseytin_iff, tseytin_not, tseytin_or};
use conjure_cp::ast::Metadata;
use conjure_cp::ast::Moo;
use conjure_cp::into_matrix_expr;

/// This function confirms that all of the input expressions are order SATInts, and returns vectors for each input of their bits
/// This function also normalizes order SATInt operands to a common value range.
pub fn validate_order_int_operands(
    exprs: Vec<Expr>,
) -> Result<(Vec<Vec<Expr>>, i32, i32), ApplicationError> {
    // Iterate over all inputs
    // Check they are order and calulate a lower and upper bound
    let mut global_min: i32 = i32::MAX;
    let mut global_max: i32 = i32::MIN;

    for operand in &exprs {
        let Expr::SATInt(_, SATIntEncoding::Order, _, (local_min, local_max)) = operand else {
            return Err(RuleNotApplicable);
        };
        global_min = global_min.min(*local_min);
        global_max = global_max.max(*local_max);
    }

    // build out by iterating over each operand and expanding it to match the new bounds
    let out: Vec<Vec<Expr>> = exprs
        .into_iter()
        .map(|expr| {
            let Expr::SATInt(_, SATIntEncoding::Order, inner, (local_min, local_max)) = expr else {
                return Err(RuleNotApplicable);
            };

            let Some(v) = inner.as_ref().clone().unwrap_list() else {
                return Err(RuleNotApplicable);
            };

            // calulcate how many trues/falses to prepend/append
            let prefix_len = (local_min - global_min) as usize;
            let postfix_len = (global_max - local_max) as usize;

            let mut bits = Vec::with_capacity(v.len() + prefix_len + postfix_len);

            // add `true`s to start
            bits.extend(std::iter::repeat_n(
                Expr::Atomic(Metadata::new(), Atom::Literal(Literal::Bool(true))),
                prefix_len,
            ));

            bits.extend(v);

            // add `false`s to end
            bits.extend(std::iter::repeat_n(
                Expr::Atomic(Metadata::new(), Atom::Literal(Literal::Bool(false))),
                postfix_len,
            ));

            Ok(bits)
        })
        .collect::<Result<_, _>>()?;

    Ok((out, global_min, global_max))
}

/// Encodes a < b for order integers.
///
/// `x < y` iff `exists i . (NOT x_i AND y_i)`
fn sat_order_lt(
    a_bits: Vec<Expr>,
    b_bits: Vec<Expr>,
    clauses: &mut Vec<conjure_cp::ast::CnfClause>,
    symbols: &mut SymbolTable,
) -> Expr {
    let mut result = Expr::Atomic(Metadata::new(), Atom::Literal(Literal::Bool(false)));

    for (a_i, b_i) in a_bits.iter().zip(b_bits.iter()) {
        // (NOT a_i AND b_i)
        let not_a_i = tseytin_not(a_i.clone(), clauses, symbols);
        let current_term = tseytin_and(&vec![not_a_i, b_i.clone()], clauses, symbols);

        // accumulate (NOT a_i AND b_i) into OR term
        result = tseytin_or(&vec![result, current_term], clauses, symbols);
    }
    result
}

/// Converts an integer literal to SATInt form
///
/// ```text
///  3
///  ~~>
///  SATInt([true;int(1..), (3, 3)])
///
/// ```
#[register_rule("SAT_Order", 9500, [Atomic])]
fn literal_sat_order_int(expr: &Expr, _: &SymbolTable) -> ApplicationResult {
    let value = {
        if let Expr::Atomic(_, Atom::Literal(Literal::Int(value))) = expr {
            *value
        } else {
            return Err(RuleNotApplicable);
        }
    };

    Ok(Reduction::pure(Expr::SATInt(
        Metadata::new(),
        SATIntEncoding::Order,
        Moo::new(into_matrix_expr!(vec![Expr::Atomic(
            Metadata::new(),
            Atom::Literal(Literal::Bool(true)),
        )])),
        (value, value),
    )))
}

/// Builds CNF for equality between two order SATInt bit-vectors.
/// This function is used by both eq and neq rules, with the output negated for neq.
/// Returns (expr, clauses, symbols).
fn sat_order_eq_expr(
    lhs_bits: &[Expr],
    rhs_bits: &[Expr],
    symbols: &SymbolTable,
) -> (Expr, Vec<conjure_cp::ast::CnfClause>, SymbolTable) {
    let bit_count = lhs_bits.len();

    let mut output = true.into();
    let mut new_symbols = symbols.clone();
    let mut new_clauses = vec![];

    for i in 0..bit_count {
        let comparison = tseytin_iff(
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

    (output, new_clauses, new_symbols)
}

/// Converts a = expression between two order SATInts to a boolean expression in cnf
///
/// ```text
/// SATInt(a) = SATInt(b) ~> Bool
/// ```
#[register_rule("SAT_Order", 9100, [Eq])]
fn eq_sat_order(expr: &Expr, symbols: &SymbolTable) -> ApplicationResult {
    let Expr::Eq(_, lhs, rhs) = expr else {
        return Err(RuleNotApplicable);
    };

    let (binding, _, _) =
        validate_order_int_operands(vec![lhs.as_ref().clone(), rhs.as_ref().clone()])?;
    let [lhs_bits, rhs_bits] = binding.as_slice() else {
        return Err(RuleNotApplicable);
    };

    let (output, new_clauses, new_symbols) = sat_order_eq_expr(lhs_bits, rhs_bits, symbols);

    Ok(Reduction::cnf(output, new_clauses, new_symbols))
}

/// Converts a != expression between two order SATInts to a boolean expression in cnf
#[register_rule("SAT_Order", 9100, [Neq])]
fn neq_sat_order(expr: &Expr, symbols: &SymbolTable) -> ApplicationResult {
    let Expr::Neq(_, lhs, rhs) = expr else {
        return Err(RuleNotApplicable);
    }; // considered covered

    let (binding, _, _) =
        validate_order_int_operands(vec![lhs.as_ref().clone(), rhs.as_ref().clone()])?;
    let [lhs_bits, rhs_bits] = binding.as_slice() else {
        return Err(RuleNotApplicable); // consider covered
    };

    let (mut output, mut new_clauses, mut new_symbols) =
        sat_order_eq_expr(lhs_bits, rhs_bits, symbols);

    output = tseytin_not(output, &mut new_clauses, &mut new_symbols);

    Ok(Reduction::cnf(output, new_clauses, new_symbols))
}

/// Converts a </>/<=/>= expression between two order SATInts to a boolean expression in cnf
///
/// ```text
/// SATInt(a) </>/<=/>= SATInt(b) ~> Bool
///
/// ```
/// Note: < and <= are rewritten by swapping operands to reuse lt logic.
#[register_rule("SAT_Order", 9100, [Lt, Gt, Leq, Geq])]
fn ineq_sat_order(expr: &Expr, symbols: &SymbolTable) -> ApplicationResult {
    let (lhs, rhs, negate) = match expr {
        // A < B -> sat_order_lt(A, B)
        Expr::Lt(_, x, y) => (x, y, false),
        // A > B -> sat_order_lt(B, A)
        Expr::Gt(_, x, y) => (y, x, false),
        // A <= B -> NOT (B < A)
        Expr::Leq(_, x, y) => (y, x, true),
        // A >= B -> NOT (A < B)
        Expr::Geq(_, x, y) => (x, y, true),
        _ => return Err(RuleNotApplicable),
    };

    let (binding, _, _) =
        validate_order_int_operands(vec![lhs.as_ref().clone(), rhs.as_ref().clone()])?;
    let [lhs_bits, rhs_bits] = binding.as_slice() else {
        return Err(RuleNotApplicable);
    };

    let mut new_symbols = symbols.clone();
    let mut new_clauses = vec![];

    let mut output = sat_order_lt(
        lhs_bits.clone(),
        rhs_bits.clone(),
        &mut new_clauses,
        &mut new_symbols,
    );

    if negate {
        output = tseytin_not(output, &mut new_clauses, &mut new_symbols);
    }

    Ok(Reduction::cnf(output, new_clauses, new_symbols))
}

/// Builds an expression for `x >= threshold` from an order-encoded SATInt bit-vector.
///
/// For a normalized range `min..=max`, input bit `b_i` encodes `x >= min + i`.
fn sat_order_geq_expr(bits: &[Expr], min: i32, max: i32, threshold: i32) -> Expr {
    if threshold <= min {
        return Expr::Atomic(Metadata::new(), Atom::Literal(Literal::Bool(true)));
    }

    if threshold > max {
        return Expr::Atomic(Metadata::new(), Atom::Literal(Literal::Bool(false)));
    }

    let idx = (threshold - min) as usize;
    bits[idx].clone()
}

/// Builds an expression for `x <= threshold` from an order-encoded SATInt bit-vector.
///
/// Uses:
/// `x <= t` <-> `NOT(x >= t + 1)`
fn sat_order_leq_expr(
    bits: &[Expr],
    min: i32,
    max: i32,
    threshold: i32,
    clauses: &mut Vec<conjure_cp::ast::CnfClause>,
    symbols: &mut SymbolTable,
) -> Expr {
    if threshold < min {
        return Expr::Atomic(Metadata::new(), Atom::Literal(Literal::Bool(false)));
    }

    if threshold >= max {
        return Expr::Atomic(Metadata::new(), Atom::Literal(Literal::Bool(true)));
    }

    let geq_next = sat_order_geq_expr(bits, min, max, threshold + 1);
    tseytin_not(geq_next, clauses, symbols)
}

/// Converts Abs of an order SATInt to an order SATInt.
///
/// ```text
/// |SATInt(a)| ~> SATInt(b)
///
/// ```
#[register_rule(("SAT_Order", 9100))]
fn abs_sat_order(expr: &Expr, symbols: &SymbolTable) -> ApplicationResult {
    let Expr::Abs(_, value_expr) = expr else {
        return Err(RuleNotApplicable);
    };

    // Validate operand and get normalized bits and bounds
    let (binding, old_min, old_max) =
        validate_order_int_operands(vec![value_expr.as_ref().clone()])?;

    // We should have exactly one operand with its bits extracted
    // This represents the bits of the input SATInt, normalized to a common range [old_min..=old_max]
    let [val_bits] = binding.as_slice() else {
        return Err(RuleNotApplicable);
    };

    // Calculate new bounds for the output absolute value:
    // The minimum absolute value is 0 if the input range includes 0, otherwise it's the smaller of the absolute values of the old bounds.
    let new_min = if old_min <= 0 && old_max >= 0 {
        0
    } else {
        old_min.abs().min(old_max.abs())
    };

    // The maximum absolute value is the larger of the absolute values of the old bounds.
    let new_max = old_min.abs().max(old_max.abs());

    let mut new_symbols = symbols.clone();
    let mut new_clauses = vec![];

    // Build each output threshold bit directly in O(1):
    //
    //   |x| >= t  <->  (x >= t) OR (x <= -t)
    //
    // Where x is the input SATInt and t is the threshold corresponding to the output bit we are building. For example, if the input range is [-2..=3] and the output range is [0..=3], the output bit for threshold 2 would be:
    //   |x| >= 2  <->  (x >= 2) OR (x <= -2)
    let mut out_bits = Vec::with_capacity((new_max - new_min + 1) as usize);
    for threshold in new_min..=new_max {
        // Build (x >= threshold) and (x <= -threshold) expressions
        let geq_threshold = sat_order_geq_expr(val_bits, old_min, old_max, threshold);
        let leq_neg_threshold = sat_order_leq_expr(
            val_bits,
            old_min,
            old_max,
            -threshold,
            &mut new_clauses,
            &mut new_symbols,
        );

        // Combine them with OR to get the output bit for this threshold
        let out_bit = tseytin_or(
            &vec![geq_threshold, leq_neg_threshold],
            &mut new_clauses,
            &mut new_symbols,
        );

        out_bits.push(out_bit);
    }

    Ok(Reduction::cnf(
        Expr::SATInt(
            Metadata::new(),
            SATIntEncoding::Order,
            Moo::new(into_matrix_expr!(out_bits)),
            (new_min, new_max),
        ),
        new_clauses,
        new_symbols,
    ))
}
