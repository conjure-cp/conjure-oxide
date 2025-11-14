#![allow(dead_code)]
use conjure_cp::ast::eval::vec_op;
use conjure_cp::ast::{
    Atom, Expression as Expr, Metadata, SymbolTable, eval_constant, run_partial_evaluator,
};
use conjure_cp::rule_engine::{
    ApplicationError::RuleNotApplicable, ApplicationResult, Reduction, register_rule,
    register_rule_set,
};
use std::sync::Arc;
use std::sync::atomic::{AtomicBool, Ordering};
use uniplate::Biplate;

register_rule_set!("Constant", ());

#[register_rule(("Base",9000))]
fn partial_evaluator(expr: &Expr, _: &SymbolTable) -> ApplicationResult {
    run_partial_evaluator(expr)
}

#[register_rule(("Constant", 9001))]
fn constant_evaluator(expr: &Expr, _: &SymbolTable) -> ApplicationResult {
    // I break the rules a bit here: this is a global rule!
    //
    // This rule is really really hot when expanding comprehensions.. Also, at time of writing, we
    // have the naive rewriter, which is slow on large trees....
    //
    // Also, constant_evaluating bottom up vs top down does things in less passes: the rewriter,
    // however, favour doing this top-down!
    //
    // e.g. or([(1=1),(2=2),(3+3 = 6)])

    if !matches!(expr, Expr::Root(_, _)) {
        return Err(RuleNotApplicable);
    };

    let has_changed: Arc<AtomicBool> = Arc::new(AtomicBool::new(false));
    let has_changed_2 = Arc::clone(&has_changed);

    let new_expr = expr.transform_bi(&move |x| {
        if let Expr::Atomic(_, Atom::Literal(_)) = x {
            return x;
        }

        match eval_constant(&x)
            .map(|c| Expr::Atomic(Metadata::new(), Atom::Literal(c)))
            .or_else(|| run_partial_evaluator(&x).ok().map(|r| r.new_expression))
        {
            Some(new_expr) => {
                has_changed.store(true, Ordering::Relaxed);
                new_expr
            }

            None => x,
        }
    });

    if has_changed_2.load(Ordering::Relaxed) {
        Ok(Reduction::pure(new_expr))
    } else {
        Err(RuleNotApplicable)
    }
}

/// Evaluate the root expression.
///
/// This returns either Expr::Root([true]) or Expr::Root([false]).
#[register_rule(("Constant", 9001))]
fn eval_root(expr: &Expr, _: &SymbolTable) -> ApplicationResult {
    // this is its own rule not part of apply_eval_constant, because root should return a new root
    // with a literal inside it, not just a literal

    let Expr::Root(_, exprs) = expr else {
        return Err(RuleNotApplicable);
    };

    match exprs.len() {
        0 => Ok(Reduction::pure(Expr::Root(
            Metadata::new(),
            vec![true.into()],
        ))),
        1 => Err(RuleNotApplicable),
        _ => {
            let lit =
                vec_op::<bool, bool>(|e| e.iter().all(|&e| e), exprs).ok_or(RuleNotApplicable)?;

            Ok(Reduction::pure(Expr::Root(
                Metadata::new(),
                vec![lit.into()],
            )))
        }
    }
}

#[cfg(test)]
mod tests {
    use conjure_cp::ast::{Expression, Moo, eval_constant};
    use conjure_cp::essence_expr;

    #[test]
    fn div_by_zero() {
        let expr = essence_expr!(1 / 0);
        assert_eq!(eval_constant(&expr), None);
    }

    #[test]
    fn safediv_by_zero() {
        let expr = Expression::SafeDiv(
            Default::default(),
            Moo::new(essence_expr!(1)),
            Moo::new(essence_expr!(0)),
        );
        assert_eq!(eval_constant(&expr), None);
    }
}
