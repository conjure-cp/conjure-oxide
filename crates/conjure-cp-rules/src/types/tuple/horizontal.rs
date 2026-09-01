use crate::guard;
use crate::shared::utils::{
    as_cmp_or_lex_op, as_eq_or_neq, collect_eq_or_neq, is_tuple_lit, tuple_expr_entries,
};
use conjure_cp::ast::{Atom, Expression as Expr, Literal, Metadata, Moo, SymbolTable};
use conjure_cp::rule_engine::ApplicationError::RuleNotApplicable;
use conjure_cp::rule_engine::{ApplicationResult, RuleEffect as Reduction, register_rule};
use conjure_cp::{bug_assert, essence_expr};

/// Index directly into an inline tuple literal, e.g. one just built by another rule (a
/// representation's own membership/equality check assembling `(key, value)` on the fly) rather
/// than read from the input model. No representation-specific rule ever sees this shape: they all
/// key off an *atomic* subject (a reference with some tuple representation, or a `Literal`
/// already folded to a constant), and a freshly-built `Expr::AbstractLiteral(_, Tuple(..))` is
/// neither -- indexing it would otherwise sit unreduced all the way to the solver backend, which
/// has no way to interpret "index N of this literal tuple expression" itself.
/// ```plain
/// (a, b, c)[2] ~> b
/// ```
#[register_rule("Base", 8700, [SafeIndex])]
fn tuple_literal_index(expr: &Expr, _: &SymbolTable) -> ApplicationResult {
    guard!(
        let Expr::SafeIndex(_, subject, indices) = expr &&
        let Some(entries) = tuple_expr_entries(subject) &&
        let Some(idx) = indices.first()                 &&
        let Expr::Atomic(_, Atom::Literal(Literal::Int(idx))) = idx
        else {
            return Err(RuleNotApplicable);
        }
    );

    let idx = (*idx - 1) as usize;
    bug_assert!(
        idx < entries.len(),
        "tuple literal indexing is out of bounds"
    );
    let selected = entries[idx].clone();

    let rest = &indices[1..];
    if rest.is_empty() {
        Ok(Reduction::pure(selected))
    } else {
        Ok(Reduction::pure(Expr::SafeIndex(
            Metadata::new(),
            Moo::new(selected),
            rest.to_vec(),
        )))
    }
}

/// Compare two inline tuple literal expressions field by field. Every existing tuple equality
/// rule (`tuple_var_eq_var`, `tuple_var_eq_lit`, ...) requires at least one side to be an atomic
/// reference with some tuple representation; two freshly-built literal expressions on both sides
/// (e.g. a representation rule comparing a rebuilt witness tuple against an input literal, neither
/// of which is a declaration) matches none of them and would otherwise reach the solver backend as
/// a raw tuple-vs-tuple equality it cannot interpret.
/// ```plain
/// (a, b) = (c, d) ~> a = c /\ b = d
/// ```
#[register_rule("Base", 8700, [Eq, Neq])]
fn tuple_literal_eq_literal(expr: &Expr, _: &SymbolTable) -> ApplicationResult {
    let (lhs, rhs, neq) = as_eq_or_neq(expr)?;
    guard!(
        let Some(lhs_entries) = tuple_expr_entries(lhs) &&
        let Some(rhs_entries) = tuple_expr_entries(rhs)
        else {
            return Err(RuleNotApplicable);
        }
    );
    bug_assert!(
        lhs_entries.len() == rhs_entries.len(),
        "equality on tuple literals with different shapes!"
    );

    let new_expr = collect_eq_or_neq(neq, lhs_entries.into_iter().zip(rhs_entries));
    Ok(Reduction::pure(new_expr))
}

/// Canonicalise chained tuple indexing before representation-specific rules see it.
///
/// ```plain
/// x[i][j] ~> x[i,j]
/// ```
#[register_rule("Bubble", 8050, [UnsafeIndex])]
fn merge_chained_tuple_indices(expr: &Expr, _: &SymbolTable) -> ApplicationResult {
    guard!(
        let Expr::UnsafeIndex(metadata, subject, outer_indices) = expr       &&
        let Expr::UnsafeIndex(_, inner_subject, inner_indices) = subject.as_ref() &&
        let Some(domain) = inner_subject.domain_of()                         &&
        domain.as_tuple().is_some()
        else {
            return Err(RuleNotApplicable);
        }
    );

    let mut indices = inner_indices.clone();
    indices.extend(outer_indices.iter().cloned());
    Ok(Reduction::pure(Expr::UnsafeIndex(
        metadata.clone(),
        inner_subject.clone(),
        indices,
    )))
}

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

#[cfg(test)]
mod tests {
    use conjure_cp::ast::{AbstractLiteral, Atom, Expression as Expr, Metadata, Moo, SymbolTable};
    use conjure_cp::rule_engine::get_rule_by_name;
    use conjure_cp::{domain_int, range};
    use uniplate::Uniplate;

    fn tuple_lit(fields: Vec<Expr>) -> Expr {
        Expr::AbstractLiteral(Metadata::new(), AbstractLiteral::Tuple(fields))
    }

    fn int_lit(n: i32) -> Expr {
        Expr::Atomic(Metadata::new(), Atom::Literal(n.into()))
    }

    #[test]
    fn tuple_literal_index_selects_the_indexed_field() {
        let subject = tuple_lit(vec![int_lit(7), int_lit(8), int_lit(9)]);
        let expr = Expr::SafeIndex(Metadata::new(), Moo::new(subject), vec![int_lit(2)]);

        let rule = get_rule_by_name("tuple_literal_index").expect("rule registered");
        let result = rule
            .apply(&expr, &SymbolTable::new())
            .expect("should index the literal");
        assert_eq!(result.new_expression, int_lit(8));
    }

    #[test]
    fn tuple_literal_index_recurses_into_a_nested_field() {
        // ((7, 8), 9)[1, 2] ~> ((7, 8))[2] ~> 8 across two fixpoint applications, the same way
        // `x[1][2]` on a represented tuple reference resolves one index per application.
        let inner = tuple_lit(vec![int_lit(7), int_lit(8)]);
        let subject = tuple_lit(vec![inner, int_lit(9)]);
        let expr = Expr::SafeIndex(
            Metadata::new(),
            Moo::new(subject),
            vec![int_lit(1), int_lit(2)],
        );

        let rule = get_rule_by_name("tuple_literal_index").expect("rule registered");
        let first = rule
            .apply(&expr, &SymbolTable::new())
            .expect("should index the outer field first");
        let Expr::SafeIndex(..) = &first.new_expression else {
            panic!(
                "expected a partially-applied SafeIndex, got {}",
                first.new_expression
            );
        };
        let second = rule
            .apply(&first.new_expression, &SymbolTable::new())
            .expect("should index the remaining field");
        assert_eq!(second.new_expression, int_lit(8));
    }

    #[test]
    fn tuple_literal_index_is_not_applicable_to_a_non_literal_subject() {
        let subject = Expr::Atomic(
            Metadata::new(),
            Atom::Reference(conjure_cp::ast::Reference::new(
                SymbolTable::new().gen_find(&domain_int!(1..3)),
            )),
        );
        let expr = Expr::SafeIndex(Metadata::new(), Moo::new(subject), vec![int_lit(1)]);

        let rule = get_rule_by_name("tuple_literal_index").expect("rule registered");
        assert!(rule.apply(&expr, &SymbolTable::new()).is_err());
    }

    #[test]
    fn tuple_literal_eq_literal_decomposes_field_by_field() {
        let lhs = tuple_lit(vec![int_lit(7), int_lit(8)]);
        let rhs = tuple_lit(vec![int_lit(7), int_lit(9)]);
        let expr = Expr::Eq(Metadata::new(), Moo::new(lhs), Moo::new(rhs));

        let rule = get_rule_by_name("tuple_literal_eq_literal").expect("rule registered");
        let result = rule
            .apply(&expr, &SymbolTable::new())
            .expect("should decompose field by field");
        assert!(matches!(result.new_expression, Expr::And(..)));
        let nodes = result.new_expression.universe();
        assert_eq!(
            nodes
                .iter()
                .filter(|node| matches!(node, Expr::Eq(..)))
                .count(),
            2
        );
    }

    /// Regression: neither `tuple_var_eq_var` nor `tuple_var_eq_lit` (both require a `Reference`
    /// on at least one side) match two freshly-built literal tuple expressions -- exactly the
    /// shape `TuplePacked`'s own field-decoding (`types::tuple::packed::vertical::unpack_digit`)
    /// produces for a compound field, compared against an input literal.
    #[test]
    fn tuple_literal_eq_literal_handles_nested_tuple_fields() {
        let inner = tuple_lit(vec![int_lit(7), Expr::from(false)]);
        let lhs = tuple_lit(vec![inner, int_lit(13)]);
        let rhs = tuple_lit(vec![
            tuple_lit(vec![int_lit(7), Expr::from(false)]),
            int_lit(13),
        ]);
        let expr = Expr::Eq(Metadata::new(), Moo::new(lhs), Moo::new(rhs));

        let rule = get_rule_by_name("tuple_literal_eq_literal").expect("rule registered");
        assert!(rule.apply(&expr, &SymbolTable::new()).is_ok());
    }

    #[test]
    fn tuple_literal_eq_literal_is_not_applicable_when_a_side_is_a_reference() {
        let lhs = Expr::Atomic(
            Metadata::new(),
            Atom::Reference(conjure_cp::ast::Reference::new(
                SymbolTable::new().gen_find(&domain_int!(1..3)),
            )),
        );
        let rhs = tuple_lit(vec![int_lit(1), int_lit(2)]);
        let expr = Expr::Eq(Metadata::new(), Moo::new(lhs), Moo::new(rhs));

        let rule = get_rule_by_name("tuple_literal_eq_literal").expect("rule registered");
        assert!(rule.apply(&expr, &SymbolTable::new()).is_err());
    }
}
