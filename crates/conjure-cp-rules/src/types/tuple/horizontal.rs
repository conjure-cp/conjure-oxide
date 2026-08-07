use crate::guard;
use crate::shared::utils::{as_cmp_or_lex_op, is_tuple_lit};
use conjure_cp::ast::{Atom, Expression as Expr, Metadata, SymbolTable};
use conjure_cp::rule_engine::ApplicationError::RuleNotApplicable;
use conjure_cp::rule_engine::{ApplicationResult, RuleEffect as Reduction, register_rule};
use conjure_cp::{bug_assert, essence_expr};

/// Convert an unsafe tuple index into a safe one.
/// ```plain
/// x[y]
/// ~>
/// { x[y] @ (y >= 1 /\ y <= |x|) }
/// ```
#[register_rule("Bubble", 8000, [UnsafeIndex])]
fn tuple_index_to_bubble(expr: &Expr, _: &SymbolTable) -> ApplicationResult {
    guard!(
        let Expr::UnsafeIndex(_, subject, indices) = expr &&
        let Some(idx) = indices.first()                   &&
        let Some(idx_dom) = idx.domain_of()               &&
        let Some(dom) = subject.domain_of()               &&
        let Some(inner_doms) = dom.as_tuple()
        else {
            return Err(RuleNotApplicable);
        }
    );
    bug_assert!(
        idx_dom.as_int().is_some(),
        "tuple indexing expression must be integer"
    );

    let len = inner_doms.len() as i32;
    let bubble_cond = essence_expr!(r"(&idx >= 1) /\ (&idx <= &len)");
    let bubble_expr = Expr::SafeIndex(Metadata::new(), subject.clone(), indices.clone());

    Ok(Reduction::pure(Expr::Bubble(
        Metadata::new(),
        bubble_expr.into(),
        bubble_cond.into(),
    )))
}

/// Put a tuple variable on the left of a comparison with a tuple literal so that either
/// representation-specific lowering can handle it.
#[register_rule("ReprGeneral", 9401, [Eq, Neq, Lt, Gt, Leq, Geq])]
fn tuple_comparison_reorder(expr: &Expr, _: &SymbolTable) -> ApplicationResult {
    guard!(
        let Some((lit, var)) = as_cmp_or_lex_op(expr)          &&
        let Expr::Atomic(_, Atom::Reference(_)) = var.as_ref() &&
        is_tuple_lit(lit.as_ref())
        else {
            return Err(RuleNotApplicable);
        }
    );

    let new_expr = match expr {
        Expr::Eq(..) => essence_expr!(&var = &lit),
        Expr::Neq(..) => essence_expr!(&var != &lit),
        Expr::Gt(..) => essence_expr!(&var < &lit),
        Expr::Lt(..) => essence_expr!(&var > &lit),
        Expr::Geq(..) => essence_expr!(&var <= &lit),
        Expr::Leq(..) => essence_expr!(&var >= &lit),
        _ => return Err(RuleNotApplicable),
    };

    Ok(Reduction::pure(new_expr))
}
