#![allow(dead_code)]
use conjure_cp::ast::eval::vec_op;
use conjure_cp::ast::{
    AbstractLiteral, Atom, Expression as Expr, Literal, Metadata, Moo, SymbolTable, eval_constant,
    run_partial_evaluator,
};
use conjure_cp::rule_engine::{
    ApplicationError::RuleNotApplicable, ApplicationResult, RuleEffect, register_rule,
    register_rule_set,
};

register_rule_set!("Constant", ());

/// Constant-folds `expr` unless doing so would inline a referenced matrix literal.
fn fold_constant_expression(expr: &Expr) -> Option<Expr> {
    let constant = eval_constant(expr)?;

    if matches!(
        (expr, &constant),
        (
            Expr::Atomic(_, Atom::Reference(_)),
            Literal::AbstractLiteral(AbstractLiteral::Matrix(_, _))
        )
    ) {
        return None;
    }

    let folded = Expr::Atomic(Metadata::new(), Atom::Literal(constant));
    if let Expr::TypeAnnotation(_, _, ty) = expr
        && let Expr::Atomic(
            _,
            Atom::Literal(Literal::AbstractLiteral(AbstractLiteral::Matrix(elems, _))),
        ) = &folded
        && elems.is_empty()
    {
        return Some(Expr::TypeAnnotation(
            Metadata::new(),
            Moo::new(folded),
            ty.clone(),
        ));
    }

    if let Expr::DomainAnnotation(_, _, domain) = expr
        && let Expr::Atomic(
            _,
            Atom::Literal(Literal::AbstractLiteral(AbstractLiteral::Matrix(elems, _))),
        ) = &folded
        && elems.is_empty()
    {
        return Some(Expr::DomainAnnotation(
            Metadata::new(),
            Moo::new(folded),
            domain.clone(),
        ));
    }

    if let Expr::Comprehension(_, comprehension) = expr
        && let Expr::Atomic(
            _,
            Atom::Literal(Literal::AbstractLiteral(AbstractLiteral::Matrix(elems, _))),
        ) = &folded
        && elems.is_empty()
        && let Some(domain) = comprehension.domain_of()
    {
        return Some(Expr::DomainAnnotation(
            Metadata::new(),
            Moo::new(folded),
            domain,
        ));
    }

    Some(folded)
}

#[register_rule(
    "Base",
    9000,
    [
        SafeIndex, InDomain, Bubble, ToInt, Abs, Sum, Product, Min, Max, Not, Or, And, Root, Imply,
        Iff, Eq, Neq, AllDiff
    ]
)]
fn partial_evaluator(expr: &Expr, _: &SymbolTable) -> ApplicationResult {
    run_partial_evaluator(expr)
}

/// Folds the focused expression when it is constant, or applies local partial evaluation.
///
/// Keep this rule local: whole-root simplification is handled by explicit root rules and by the
/// worklist rechecking ancestors after child rewrites.
#[register_rule("Constant", 9001)]
fn constant_evaluator(expr: &Expr, _: &SymbolTable) -> ApplicationResult {
    match expr {
        // Focused `AbstractLiteral` nodes must still fold locally: parent rules often match the
        // `Atomic(Literal(...))` form after the worklist revisits them.
        Expr::Atomic(_, Atom::Literal(conjure_cp::ast::Literal::AbstractLiteral(_))) => {
            Err(RuleNotApplicable)
        }
        _ => match fold_constant_expression(expr)
            .or_else(|| run_partial_evaluator(expr).ok().map(|r| r.new_expression))
        {
            Some(new_expr) if &new_expr != expr => Ok(RuleEffect::pure(new_expr)),
            _ => Err(RuleNotApplicable),
        },
    }
}

/// Evaluate the root expression.
///
/// This returns either Expr::Root([true]) or Expr::Root([false]).
#[register_rule("Constant", 9001, [Root])]
fn eval_root(expr: &Expr, _: &SymbolTable) -> ApplicationResult {
    // this is its own rule not part of apply_eval_constant, because root should return a new root
    // with a literal inside it, not just a literal

    let Expr::Root(_, exprs) = expr else {
        return Err(RuleNotApplicable);
    };

    match exprs.len() {
        0 => Ok(RuleEffect::pure(Expr::Root(
            Metadata::new(),
            vec![true.into()],
        ))),
        1 => Err(RuleNotApplicable),
        _ => {
            let lit =
                vec_op::<bool, bool>(|e| e.iter().all(|&e| e), exprs).ok_or(RuleNotApplicable)?;

            Ok(RuleEffect::pure(Expr::Root(
                Metadata::new(),
                vec![lit.into()],
            )))
        }
    }
}
