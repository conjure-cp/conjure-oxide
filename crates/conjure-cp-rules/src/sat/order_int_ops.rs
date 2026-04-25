use conjure_cp::ast::{Atom, CnfClause, Expression as Expr, Literal};
use conjure_cp::ast::{SATIntEncoding, SymbolTable};
use conjure_cp::rule_engine::ApplicationError;
use conjure_cp::rule_engine::{
    ApplicationError::RuleNotApplicable, ApplicationResult, Reduction, register_rule,
};

use crate::sat::boolean::{tseytin_and, tseytin_iff, tseytin_not, tseytin_or};
use crate::sat::direct_int_ops::floor_div;
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

/// Converts a / expression between two order SATInts to a new order SATInt
/// using the "lookup table" method.
///
/// ```text
/// SafeDiv(SATInt(a), SATInt(b)) ~> SATInt(c)
///
/// ```
#[register_rule("SAT_Order", 9100, [SafeDiv])]
fn safediv_sat_order(expr: &Expr, symbols: &SymbolTable) -> ApplicationResult {
    let Expr::SafeDiv(_, numer_expr, denom_expr) = expr else {
        return Err(RuleNotApplicable);
    };

    let Expr::SATInt(_, SATIntEncoding::Order, numer_inner, (numer_min, numer_max)) =
        numer_expr.as_ref()
    else {
        return Err(RuleNotApplicable);
    };
    let Some(numer_bits) = numer_inner.as_ref().clone().unwrap_list() else {
        return Err(RuleNotApplicable);
    };

    let Expr::SATInt(_, SATIntEncoding::Order, denom_inner, (denom_min, denom_max)) =
        denom_expr.as_ref()
    else {
        return Err(RuleNotApplicable);
    };

    let Some(denom_bits) = denom_inner.as_ref().clone().unwrap_list() else {
        return Err(RuleNotApplicable);
    };

    let mut quot_min = i32::MAX;
    let mut quot_max = i32::MIN;

    for i in *numer_min..=*numer_max {
        for j in *denom_min..=*denom_max {
            let k = if j == 0 { 0 } else { i / j };
            quot_min = quot_min.min(k);
            quot_max = quot_max.max(k);
        }
    }

    let mut new_symbols = symbols.clone();
    let mut quot_bits = Vec::new();

    // generate boolean variables for all possible quotients
    for _ in quot_min..=quot_max {
        let decl = new_symbols.gen_find(&conjure_cp::ast::Domain::bool());
        quot_bits.push(Expr::Atomic(
            Metadata::new(),
            Atom::Reference(conjure_cp::ast::Reference::new(decl)),
        ));
    }

    let mut new_clauses = vec![];

    // Generate the lookup table clauses, and extract exact bounds
    for i in *numer_min..=*numer_max {
        let n_idx = (i - numer_min) as usize;
        let n_bit = &numer_bits[n_idx];
        let n_next_bit = if i < *numer_max {
            Some(&numer_bits[n_idx + 1])
        } else {
            None
        };

        for j in *denom_min..=*denom_max {
            let d_idx = (j - denom_min) as usize;
            let d_bit = &denom_bits[d_idx];
            let d_next_bit = if j < *denom_max {
                Some(&denom_bits[d_idx + 1])
            } else {
                None
            };

            let k = if j == 0 { 0 } else { floor_div(i, j) };

            // we needed to represent (n >= i) and not (n >= i+1) or ((d >= j) and not (d >= j+1))
            // using or's so using de-morgans we do
            // NOT(N >= i) OR (N >= i+1) OR NOT(D >= j) OR (D >= j+1)
            let mut base_cond = vec![Expr::Not(Metadata::new(), Moo::new(n_bit.clone()))];
            if let Some(n_next) = n_next_bit {
                base_cond.push(n_next.clone());
            }

            base_cond.push(Expr::Not(Metadata::new(), Moo::new(d_bit.clone())));
            if let Some(d_next) = d_next_bit {
                base_cond.push(d_next.clone());
            }

            // force the quotient to represent exactly value 'k'
            for m in quot_min..=quot_max {
                let q_idx = (m - quot_min) as usize;
                let q_bit = &quot_bits[q_idx];

                let mut clause = base_cond.clone();
                if m <= k {
                    // the value is at least m, so bit (Q >= m) is True
                    clause.push(q_bit.clone());
                } else {
                    // the value is less than m, so bit (Q >= m) is False
                    clause.push(Expr::Not(Metadata::new(), Moo::new(q_bit.clone())));
                }

                // push (NOT condition) OR quotient
                new_clauses.push(conjure_cp::ast::CnfClause::new(clause));
            }
        }
    }

    let quot_int = Expr::SATInt(
        Metadata::new(),
        SATIntEncoding::Order,
        Moo::new(into_matrix_expr!(quot_bits)),
        (quot_min, quot_max),
    );
    Ok(Reduction::cnf(quot_int, new_clauses, new_symbols))
}
