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
#[register_rule(("SAT_Order", 9500))]
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

/// Converts a = expression between two order SATInts to a boolean expression in cnf
///
/// ```text
/// SATInt(a) = SATInt(b) ~> Bool
/// ```
#[register_rule(("SAT_Order", 9100))]
fn eq_sat_order(expr: &Expr, symbols: &SymbolTable) -> ApplicationResult {
    let Expr::Eq(_, lhs, rhs) = expr else {
        return Err(RuleNotApplicable);
    };

    let (binding, _, _) =
        validate_order_int_operands(vec![lhs.as_ref().clone(), rhs.as_ref().clone()])?;
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

/// Converts a </>/<=/>= expression between two order SATInts to a boolean expression in cnf
///
/// ```text
/// SATInt(a) </>/<=/>= SATInt(b) ~> Bool
///
/// ```
/// Note: < and <= are rewritten by swapping operands to reuse lt logic.
#[register_rule(("SAT_Order", 9100))]
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

/// Converts a - expression for a SATInt to a new SATInt
/// 
/// ```text
/// -SATInt(a) ~> SATInt(b)
/// 
/// ```
#[register_rule(("SAT_Order", 9100))]
fn neg_sat_order(expr: &Expr, symbols: &SymbolTable) -> ApplicationResult {
    let Expr::Neg(_, value) = expr else {
        return Err(RuleNotApplicable);
    };

    let (binding, old_min, old_max) =
        validate_order_int_operands(vec![value.as_ref().clone()])?;
    let [val_bits] = binding.as_slice() else {
        return Err(RuleNotApplicable);
    };

    let new_min = -old_max;
    let new_max = -old_min;

    let n = val_bits.len();
    let mut out: Vec<Expr> = Vec::with_capacity(n);

    let mut new_symbols = symbols.clone();
    let mut new_clauses = vec![];

    let ff = Expr::Atomic(Metadata::new(), Atom::Literal(Literal::Bool(false)));
    for i in 0..n {
        let idx = n - i;
        let src = if idx == n {
            ff.clone()
        } else {
            val_bits[idx].clone()
        };

        let neg_bit = tseytin_not(src, &mut new_clauses, &mut new_symbols);
        out.push(neg_bit);
    }

    Ok(Reduction::cnf(
        Expr::SATInt(
            Metadata::new(),
            SATIntEncoding::Order,
            Moo::new(into_matrix_expr!(out)),
            (new_min, new_max),
        ),
        new_clauses,
        new_symbols
    ))
}

