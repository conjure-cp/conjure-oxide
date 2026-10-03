use conjure_cp::ast::{Atom, Expression as Expr, Literal};
use conjure_cp::ast::{SATIntEncoding, SymbolTable};
use conjure_cp::rule_engine::ApplicationError;
use conjure_cp::rule_engine::{
    ApplicationError::RuleNotApplicable, ApplicationResult, RuleEffect, register_rule,
};

use crate::backends::sat::boolean::tseytin_not;
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

            let Some(v) = inner.as_ref().clone().into_list() else {
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

/// Converts a - expression for a SATInt to a new SATInt
///
/// ```text
/// -SATInt(a) ~> SATInt(b)
///
/// ```
#[register_rule("SAT", 9100, [Neg])]
fn neg_sat_order(expr: &Expr, symbols: &SymbolTable) -> ApplicationResult {
    let Expr::Neg(_, value) = expr else {
        return Err(RuleNotApplicable);
    };

    let (binding, old_min, old_max) = validate_order_int_operands(vec![value.as_ref().clone()])?;
    let [val_bits] = binding.as_slice() else {
        return Err(RuleNotApplicable); // consider covered
    };

    let new_min = -old_max;
    let new_max = -old_min;

    let n = val_bits.len();
    let mut out: Vec<Expr> = Vec::with_capacity(n);

    let mut new_symbols = symbols.clone();
    let mut new_sat_decisions = vec![];

    let ff = Expr::Atomic(Metadata::new(), Atom::Literal(Literal::Bool(false)));
    out.push(tseytin_not(ff, &mut new_sat_decisions, &mut new_symbols));

    for i in 1..n {
        let src = val_bits[n - i].clone();
        let neg_bit = tseytin_not(src, &mut new_sat_decisions, &mut new_symbols);
        out.push(neg_bit);
    }

    Ok(RuleEffect::sat(
        Expr::SATInt(
            Metadata::new(),
            SATIntEncoding::Order,
            Moo::new(into_matrix_expr!(out)),
            (new_min, new_max),
        ),
        new_sat_decisions,
        new_symbols,
    ))
}
