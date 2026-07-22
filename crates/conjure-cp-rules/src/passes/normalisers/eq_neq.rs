//! Normalising rules for `Neq` and `Eq`.

use conjure_cp::ast::{Expression as Expr, SymbolTable, Typeable, try_lower_bool_atom_eq_true};
use conjure_cp::rule_engine::{
    ApplicationError::RuleNotApplicable, ApplicationResult, RuleEffect, register_rule,
};

use conjure_cp::ast::ReturnType::{Matrix, Set};
use conjure_cp::essence_expr;

/// Normalises boolean `x = true` to `x` before Minion flattening of nested equalities.
///
/// ```text
/// x = true  ~>  x
/// true = x  ~>  x
/// ```
/// where `x` is a non-literal boolean atom.
///
/// The same lowering is applied by the partial evaluator's `Eq` arm; this rule remains as an
/// explicit Base-priority oracle for any sites that still reach ordinary rewriting.
#[register_rule("Base", 9000, [Eq])]
fn bool_atom_eq_true(expr: &Expr, _: &SymbolTable) -> ApplicationResult {
    try_lower_bool_atom_eq_true(expr)
        .map(RuleEffect::pure)
        .ok_or(RuleNotApplicable)
}

/// Converts a negated `Neq` to an `Eq`
///
/// ```text
/// not(neq(x)) ~> eq(x)
/// ```
#[register_rule("Base", 8800, [Not])]
fn negated_neq_to_eq(expr: &Expr, _: &SymbolTable) -> ApplicationResult {
    match expr {
        Expr::Not(_, a) => match a.as_ref() {
            Expr::Neq(_, b, c) if (b.is_safe() && c.is_safe()) => {
                Ok(RuleEffect::pure(essence_expr!(&b = &c)))
            }
            _ => Err(RuleNotApplicable),
        },
        _ => Err(RuleNotApplicable),
    }
}

/// Converts a negated `Eq` to an `Neq`
///
/// ```text
/// not(eq(x)) ~> neq(x)
/// ```
/// don't want this to apply to sets
///
/// Also can't apply to matrices, since undefinedness between two matrices with different domains
/// causes a != b to actually have a different meaning than !(a = b)
#[register_rule("Base", 8800, [Not])]
fn negated_eq_to_neq(expr: &Expr, _: &SymbolTable) -> ApplicationResult {
    match expr {
        Expr::Not(_, a) => match a.as_ref() {
            Expr::Eq(_, b, c) if (b.is_safe() && c.is_safe()) => {
                if matches!(b.as_ref().return_type(), Set(_) | Matrix(_)) {
                    return Err(RuleNotApplicable);
                }
                if matches!(c.as_ref().return_type(), Set(_) | Matrix(_)) {
                    return Err(RuleNotApplicable);
                }
                Ok(RuleEffect::pure(essence_expr!(&b != &c)))
            }
            _ => Err(RuleNotApplicable),
        },
        _ => Err(RuleNotApplicable),
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use conjure_cp::ast::{
        Atom, DeclarationPtr, Domain, Literal as Lit, Metadata, Moo, Name, Range, Reference,
        run_partial_evaluator,
    };
    use conjure_cp::rule_engine::{ApplicationError, get_rule_by_name};

    /// Boolean decision-variable atomic expression.
    fn bool_atom(name: &str) -> Expr {
        Expr::Atomic(
            Metadata::new(),
            Atom::Reference(Reference::new(DeclarationPtr::new_find(
                Name::user(name),
                Domain::bool(),
            ))),
        )
    }

    /// Boolean literal atomic expression.
    fn bool_lit(value: bool) -> Expr {
        Expr::Atomic(Metadata::new(), Atom::Literal(Lit::Bool(value)))
    }

    #[test]
    fn bool_atom_eq_true_lowers_variable_equals_true() {
        let x = bool_atom("x");
        let expr = Expr::Eq(
            Metadata::new(),
            Moo::new(x.clone()),
            Moo::new(bool_lit(true)),
        );
        let lowered = try_lower_bool_atom_eq_true(&expr).expect("should lower");
        assert_eq!(lowered, x);

        let rule = get_rule_by_name("bool_atom_eq_true").expect("rule registered");
        let result = rule
            .apply(&expr, &SymbolTable::new())
            .expect("rule applies");
        assert_eq!(result.new_expression, x);

        let pe = run_partial_evaluator(&expr).expect("partial evaluator Eq arm lowers");
        assert_eq!(pe.new_expression, x);
    }

    #[test]
    fn bool_atom_eq_true_lowers_true_equals_variable() {
        let x = bool_atom("x");
        let expr = Expr::Eq(
            Metadata::new(),
            Moo::new(bool_lit(true)),
            Moo::new(x.clone()),
        );
        let lowered = try_lower_bool_atom_eq_true(&expr).expect("should lower");
        assert_eq!(lowered, x);
    }

    #[test]
    fn bool_atom_eq_true_refuses_false_equality() {
        let expr = Expr::Eq(
            Metadata::new(),
            Moo::new(bool_atom("x")),
            Moo::new(bool_lit(false)),
        );
        assert!(try_lower_bool_atom_eq_true(&expr).is_none());
        let rule = get_rule_by_name("bool_atom_eq_true").expect("rule registered");
        let err = rule.apply(&expr, &SymbolTable::new()).unwrap_err();
        assert!(matches!(err, ApplicationError::RuleNotApplicable));
    }

    #[test]
    fn bool_atom_eq_true_refuses_integer_atom() {
        let int_atom = Expr::Atomic(
            Metadata::new(),
            Atom::Reference(Reference::new(DeclarationPtr::new_find(
                Name::user("n"),
                Domain::int(vec![Range::Bounded(1, 3)]),
            ))),
        );
        let expr = Expr::Eq(
            Metadata::new(),
            Moo::new(int_atom),
            Moo::new(bool_lit(true)),
        );
        assert!(try_lower_bool_atom_eq_true(&expr).is_none());
    }
}
