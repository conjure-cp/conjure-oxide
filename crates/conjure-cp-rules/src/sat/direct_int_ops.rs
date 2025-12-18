use conjure_cp::ast::{Atom, Expression as Expr, Literal};
use conjure_cp::ast::{SATIntEncoding, SymbolTable};
use conjure_cp::rule_engine::ApplicationError;
use conjure_cp::rule_engine::{
    ApplicationError::RuleNotApplicable, ApplicationResult, Reduction, register_rule,
};

use conjure_cp::ast::AbstractLiteral::Matrix;
use conjure_cp::ast::Metadata;
use conjure_cp::ast::Moo;
use conjure_cp::into_matrix_expr;

use itertools::Itertools;

use super::boolean::{
    tseytin_and, tseytin_iff, tseytin_imply, tseytin_mux, tseytin_not, tseytin_or, tseytin_xor,
};
use super::integer_repr::{bit_magnitude, match_bits_length, validate_log_int_operands};

use conjure_cp::ast::CnfClause;

use std::cmp;

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
/// This function also extends all inputs to have the same range, and returns the new range
pub fn validate_direct_int_operands(
    exprs: Vec<Expr>,
) -> Result<(Vec<Vec<Expr>>, i32, i32), ApplicationError> {
    // TODO: In the future it may be possible to optimize operations between integers with different bit sizes
    // Collect inner bit vectors from each SATInt

    // Iterate over all inputs
    // Check they are direct and calulate a lower and upper bound
    let mut global_min: i32 = std::i32::MAX;
    let mut global_max: i32 = std::i32::MIN;

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
            bits.extend(
                std::iter::repeat(Expr::Atomic(
                    Metadata::new(),
                    Atom::Literal(Literal::Bool(false)),
                ))
                .take(prefix_len),
            );

            bits.extend(v);

            // add 0s to end
            bits.extend(
                std::iter::repeat(Expr::Atomic(
                    Metadata::new(),
                    Atom::Literal(Literal::Bool(false)),
                ))
                .take(postfix_len),
            );

            Ok(bits)
        })
        .collect::<Result<_, _>>()?;

    Ok((out, global_min, global_max))
}

/// Converts a = expression between two direct SATInts to a boolean expression in cnf
///
/// ```text
/// SATInt(a) = SATInt(b) ~> Bool
///
/// ```
#[register_rule(("SAT_Direct", 9100))]
fn eq_sat_direct(expr: &Expr, symbols: &SymbolTable) -> ApplicationResult {
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
#[register_rule(("SAT_Direct", 9100))]
fn neq_sat_direct(expr: &Expr, symbols: &SymbolTable) -> ApplicationResult {
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
