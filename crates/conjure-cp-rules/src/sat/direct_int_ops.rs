use conjure_cp::ast::{Atom, Expression as Expr, Literal};
use conjure_cp::ast::{SATIntEncoding, SymbolTable};
use conjure_cp::rule_engine::ApplicationError;
use conjure_cp::rule_engine::{
    ApplicationError::RuleNotApplicable, ApplicationResult, Reduction, register_rule,
};

use conjure_cp::ast::Metadata;
use conjure_cp::ast::Moo;
use conjure_cp::into_matrix_expr;

use super::boolean::{tseytin_and, tseytin_iff, tseytin_not, tseytin_or, tseytin_xor};

use conjure_cp::ast::CnfClause;

/// Converts an integer literal to SATInt form
///
/// ```text
///  3
///  ~~>
///  SATInt([true;int(1..), (3, 3)])
///
/// ```
#[register_rule(("SAT_Direct", 9500))]
fn literal_sat_direct_int(expr: &Expr, _: &SymbolTable) -> ApplicationResult {
    let value = {
        if let Expr::Atomic(_, Atom::Literal(Literal::Int(value))) = expr {
            *value
        } else {
            return Err(RuleNotApplicable);
        }
    };

    Ok(Reduction::pure(Expr::SATInt(
        Metadata::new(),
        SATIntEncoding::Direct,
        Moo::new(into_matrix_expr!(vec![Expr::Atomic(
            Metadata::new(),
            Atom::Literal(Literal::Bool(true)),
        )])),
        (value, value),
    )))
}

/// This function confirms that all of the input expressions are direct SATInts, and returns vectors for each input of their bits
/// This function also normalizes direct SATInt operands to a common value range by zero-padding.
pub fn validate_direct_int_operands(
    exprs: Vec<Expr>,
) -> Result<(Vec<Vec<Expr>>, i32, i32), ApplicationError> {
    // TODO: In the future it may be possible to optimize operations between integers with different bit sizes
    // Collect inner bit vectors from each SATInt

    // Iterate over all inputs
    // Check they are direct and calulate a lower and upper bound
    let mut global_min: i32 = i32::MAX;
    let mut global_max: i32 = i32::MIN;

    for operand in &exprs {
        let Expr::SATInt(_, SATIntEncoding::Direct, _, (local_min, local_max)) = operand else {
            return Err(RuleNotApplicable);
        };
        global_min = global_min.min(*local_min);
        global_max = global_max.max(*local_max);
    }

    // build out by iterating over each operand and expanding it to match the new bounds

    let out: Vec<Vec<Expr>> = exprs
        .into_iter()
        .map(|expr| {
            let Expr::SATInt(_, SATIntEncoding::Direct, inner, (local_min, local_max)) = expr
            else {
                return Err(RuleNotApplicable);
            };

            let Some(v) = inner.as_ref().clone().unwrap_list() else {
                return Err(RuleNotApplicable);
            };

            // calulcate how many zeroes to prepend/append
            let prefix_len = (local_min - global_min) as usize;
            let postfix_len = (global_max - local_max) as usize;

            let mut bits = Vec::with_capacity(v.len() + prefix_len + postfix_len);

            // add 0s to start
            bits.extend(std::iter::repeat_n(
                Expr::Atomic(Metadata::new(), Atom::Literal(Literal::Bool(false))),
                prefix_len,
            ));

            bits.extend(v);

            // add 0s to end
            bits.extend(std::iter::repeat_n(
                Expr::Atomic(Metadata::new(), Atom::Literal(Literal::Bool(false))),
                postfix_len,
            ));

            Ok(bits)
        })
        .collect::<Result<_, _>>()?;

    Ok((out, global_min, global_max))
}

/// Converts a = expression between two direct SATInts to a boolean expression in cnf
///
/// ```text
/// SATInt(a) = SATInt(b) ~> Bool
/// ```
/// NOTE: This rule reduces to AND_i (a[i] ≡ b[i]) and does not enforce one-hotness.
#[register_rule(("SAT_Direct", 9100))]
fn eq_sat_direct(expr: &Expr, symbols: &SymbolTable) -> ApplicationResult {
    // TODO: this could be optimized by just going over the sections of both vectors where the ranges intersect
    // this does require enforcing structure separately
    let Expr::Eq(_, lhs, rhs) = expr else {
        return Err(RuleNotApplicable);
    };

    let (binding, _, _) =
        validate_direct_int_operands(vec![lhs.as_ref().clone(), rhs.as_ref().clone()])?;
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

/// Converts a != expression between two direct SATInts to a boolean expression in cnf
///
/// ```text
/// SATInt(a) != SATInt(b) ~> Bool
///
/// ```
///
/// True iff at least one value position differs.
#[register_rule(("SAT_Direct", 9100))]
fn neq_sat_direct(expr: &Expr, symbols: &SymbolTable) -> ApplicationResult {
    let Expr::Neq(_, lhs, rhs) = expr else {
        return Err(RuleNotApplicable);
    };

    let (binding, _, _) =
        validate_direct_int_operands(vec![lhs.as_ref().clone(), rhs.as_ref().clone()])?;
    let [lhs_bits, rhs_bits] = binding.as_slice() else {
        return Err(RuleNotApplicable);
    };

    let bit_count = lhs_bits.len();

    let mut output = false.into();
    let mut new_symbols = symbols.clone();
    let mut new_clauses = vec![];
    let mut comparison;

    for i in 0..bit_count {
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

/// Converts a </>/<=/>= expression between two direct SATInts to a boolean expression in cnf
///
/// ```text
/// SATInt(a) </>/<=/>= SATInt(b) ~> Bool
///
/// ```
/// Note: < and <= are rewritten by swapping operands to reuse lt logic.
#[register_rule(("SAT", 9100))]
fn ineq_sat_direct(expr: &Expr, symbols: &SymbolTable) -> ApplicationResult {
    let (lhs, rhs, negate) = match expr {
        // A < B -> sat_direct_lt(A, B)
        Expr::Lt(_, x, y) => (x, y, false),
        // A > B -> sat_direct_lt(B, A)
        Expr::Gt(_, x, y) => (y, x, false),
        // A <= B -> NOT (B < A)
        Expr::Leq(_, x, y) => (y, x, true),
        // A >= B -> NOT (A < B)
        Expr::Geq(_, x, y) => (x, y, true),
        _ => return Err(RuleNotApplicable),
    };

    let (binding, _, _) =
        validate_direct_int_operands(vec![lhs.as_ref().clone(), rhs.as_ref().clone()])?;
    let [lhs_bits, rhs_bits] = binding.as_slice() else {
        return Err(RuleNotApplicable);
    };

    let mut new_symbols = symbols.clone();
    let mut new_clauses = vec![];

    let mut output = sat_direct_lt(
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

/// Encodes a < b for one-hot direct integers using prefix OR logic.
fn sat_direct_lt(
    a: Vec<Expr>,
    b: Vec<Expr>,
    clauses: &mut Vec<CnfClause>,
    symbols: &mut SymbolTable,
) -> Expr {
    let mut b_or = Expr::Atomic(Metadata::new(), Atom::Literal(Literal::Bool(false)));
    let mut cum_result = Expr::Atomic(Metadata::new(), Atom::Literal(Literal::Bool(false)));

    for (a_i, b_i) in a.iter().zip(b.iter()) {
        // b_or is prefix_or of b up to index i: B_i = b_0 | ... | b_i
        b_or = tseytin_or(&vec![b_or, b_i.clone()], clauses, symbols);

        // a < b if there exists i such that a=i and b > i.
        // b > i is equivalent to NOT(B_i) assuming one-hotness.
        let not_b_or = tseytin_not(b_or.clone(), clauses, symbols);
        let a_i_and_not_b_i = tseytin_and(&vec![a_i.clone(), not_b_or], clauses, symbols);

        cum_result = tseytin_or(&vec![cum_result, a_i_and_not_b_i], clauses, symbols);
    }

    cum_result
}

/// Converts a - expression for a SATInt to a new SATInt
///
/// ```text
/// -SATInt(a) ~> SATInt(b)
///
/// ```
#[register_rule(("SAT_Direct", 9100))]
fn neg_sat_direct(expr: &Expr, _: &SymbolTable) -> ApplicationResult {
    let Expr::Neg(_, value) = expr else {
        return Err(RuleNotApplicable);
    };

    let (binding, old_min, old_max) = validate_direct_int_operands(vec![value.as_ref().clone()])?;
    let [val_bits] = binding.as_slice() else {
        return Err(RuleNotApplicable);
    };

    let new_min = -old_max;
    let new_max = -old_min;

    let mut out = val_bits.clone();
    out.reverse();

    Ok(Reduction::pure(Expr::SATInt(
        Metadata::new(),
        SATIntEncoding::Direct,
        Moo::new(into_matrix_expr!(out)),
        (new_min, new_max),
    )))
}
