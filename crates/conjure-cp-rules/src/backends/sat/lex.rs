//! Scalar lexicographic comparisons reuse numeric relation and Boolean decisions.
use crate::types::record::RecordComponents;
use crate::types::tuple::{TupleComponents, TuplePacked};
use conjure_cp::ast::{Atom, Expression, Metadata, Moo, SymbolTable};
use conjure_cp::rule_engine::{
    ApplicationError, ApplicationError::RuleNotApplicable, ApplicationResult, RuleEffect,
    register_rule,
};

/// The scalars a lex entry contributes: a represented tuple or record contributes its fields in
/// order, since its own ordering is lexicographic over them, and a packed tuple its single
/// order-preserving integer. Returns `None` for an entry that is already a single element.
fn lex_entry_fields(entry: &Expression) -> Option<Vec<Expression>> {
    let Expression::Atomic(_, Atom::Reference(reference)) = entry else {
        return None;
    };
    let declaration = reference.ptr();
    let fields = if let Some(repr) = declaration.get_repr::<TupleComponents>() {
        repr.field_exprs()
    } else if let Some(repr) = declaration.get_repr::<RecordComponents>() {
        repr.field_exprs()
    } else {
        vec![declaration.get_repr::<TuplePacked>()?.packed_expr()]
    };
    Some(
        fields
            .into_iter()
            .flat_map(|field| lex_entry_fields(&field).unwrap_or_else(|| vec![field]))
            .collect(),
    )
}

fn is_scalar(entry: &Expression) -> bool {
    entry
        .domain_of()
        .is_some_and(|domain| domain.is_int() || domain.is_bool())
}

/// Comparing `[t1, t2]` with `[u1, u2]` lexicographically equals comparing their fields
/// `[t1.a, t1.b, t2.a, t2.b]` with `[u1.a, u1.b, u2.a, u2.b]`: entries of one type have one width.
/// Runs after `tuple_var_cmp_var`, which already handles a comparison of single tuples.
#[register_rule("SAT", 9300, [LexLt, LexLeq])]
fn splice_compound_lex_entries(expr: &Expression, _: &SymbolTable) -> ApplicationResult {
    let (lhs, rhs) = match expr {
        Expression::LexLt(_, lhs, rhs) | Expression::LexLeq(_, lhs, rhs) => (lhs, rhs),
        _ => return Err(RuleNotApplicable),
    };
    let mut spliced = false;
    let mut splice = |operand: &Expression| -> Result<Moo<Expression>, ApplicationError> {
        let mut scalars = Vec::new();
        for entry in super::counting::matrix_entries(operand).ok_or(RuleNotApplicable)? {
            match lex_entry_fields(&entry) {
                Some(fields) => {
                    spliced = true;
                    scalars.extend(fields);
                }
                // Splicing one side while the other still holds an unexpanded tuple (say, a
                // pending index) would misalign the fields, so only scalars stay as they are.
                None if is_scalar(&entry) => scalars.push(entry),
                None => return Err(RuleNotApplicable),
            }
        }
        Ok(Moo::new(conjure_cp::into_matrix_expr!(scalars)))
    };
    let (lhs, rhs) = (splice(lhs)?, splice(rhs)?);
    if !spliced {
        return Err(RuleNotApplicable);
    }
    Ok(RuleEffect::pure(match expr {
        Expression::LexLt(..) => Expression::LexLt(Metadata::new(), lhs, rhs),
        _ => Expression::LexLeq(Metadata::new(), lhs, rhs),
    }))
}

#[register_rule("SAT", 18500, [LexLt, LexLeq])]
fn scalar_lex_comparison(expr: &Expression, _: &SymbolTable) -> ApplicationResult {
    let (lhs, rhs) = match expr {
        Expression::LexLt(_, lhs, rhs) | Expression::LexLeq(_, lhs, rhs) => (lhs, rhs),
        _ => return Err(RuleNotApplicable),
    };
    let elements = |operand: &Expression| {
        // Explicit matrices retain element order even after integer representation, and
        // lexicographic order does not depend on their index values.
        super::counting::matrix_entries(operand)
            .map(Ok)
            .unwrap_or_else(|| crate::shared::lex::lex_operand_elements(operand))
    };
    let (lhs, rhs) = (elements(lhs)?, elements(rhs)?);
    for entries in [&lhs, &rhs] {
        if entries.iter().any(|entry| !is_scalar(entry)) {
            // Compound comparisons promote back to lex; their representation rules must run first.
            return Err(RuleNotApplicable);
        }
    }
    Ok(RuleEffect::pure(
        crate::shared::lex::lex_elements_to_recursive_or(
            &lhs,
            &rhs,
            matches!(expr, Expression::LexLeq(..)),
        ),
    ))
}

/// Representation ordering constraints may already be flattened into scalar atoms.
#[register_rule("SAT", 18500, [FlatLexLt, FlatLexLeq])]
fn flat_scalar_lex_comparison(expr: &Expression, symbols: &SymbolTable) -> ApplicationResult {
    let (left, right, strict) = match expr {
        Expression::FlatLexLt(_, left, right) => (left, right, true),
        Expression::FlatLexLeq(_, left, right) => (left, right, false),
        _ => return Err(RuleNotApplicable),
    };
    let matrix = |atoms: &[conjure_cp::ast::Atom]| {
        Moo::new(conjure_cp::into_matrix_expr!(
            atoms
                .iter()
                .cloned()
                .map(Expression::from)
                .collect::<Vec<_>>()
        ))
    };
    let expression = if strict {
        Expression::LexLt(Metadata::new(), matrix(left), matrix(right))
    } else {
        Expression::LexLeq(Metadata::new(), matrix(left), matrix(right))
    };
    scalar_lex_comparison(&expression, symbols)
}

/// A lex comparison whose entries include compound values (a set, say) compares the first
/// entries, then the rest: `a <lex b` iff `a[1] < b[1]`, or `a[1] = b[1]` and the tails compare
/// the same way. A strict comparison of single entries is the entries' own order, which their
/// representation rules provide, so it is left to them.
#[register_rule("SAT", 9200, [LexLt, LexLeq])]
fn decompose_compound_lex(expr: &Expression, _: &SymbolTable) -> ApplicationResult {
    let (lhs, rhs, strict) = match expr {
        Expression::LexLt(_, lhs, rhs) => (lhs, rhs, true),
        Expression::LexLeq(_, lhs, rhs) => (lhs, rhs, false),
        _ => return Err(RuleNotApplicable),
    };
    let lhs = super::counting::matrix_entries(lhs).ok_or(RuleNotApplicable)?;
    let rhs = super::counting::matrix_entries(rhs).ok_or(RuleNotApplicable)?;
    // Wait for every entry to be a variable or a scalar, so representation rules see references.
    let ready = |entry: &Expression| is_scalar(entry) || matches!(entry, Expression::Atomic(..));
    if lhs.iter().chain(&rhs).all(is_scalar)
        || !lhs.iter().chain(&rhs).all(ready)
        || (strict && lhs.len() == 1 && rhs.len() == 1)
    {
        return Err(RuleNotApplicable);
    }
    Ok(RuleEffect::pure(lex_prefix(&lhs, &rhs, strict)))
}

fn lex_prefix(lhs: &[Expression], rhs: &[Expression], strict: bool) -> Expression {
    let (Some((a, lhs_rest)), Some((b, rhs_rest))) = (lhs.split_first(), rhs.split_first()) else {
        // An empty list is smaller than a non-empty one, and equal to an empty one.
        return Expression::from(lhs.is_empty() && !(strict && rhs.is_empty()));
    };
    let single = |entry: &Expression| Moo::new(conjure_cp::into_matrix_expr!(vec![entry.clone()]));
    Expression::Or(
        Metadata::new(),
        Moo::new(conjure_cp::into_matrix_expr!(vec![
            Expression::LexLt(Metadata::new(), single(a), single(b)),
            Expression::And(
                Metadata::new(),
                Moo::new(conjure_cp::into_matrix_expr!(vec![
                    Expression::Eq(Metadata::new(), Moo::new(a.clone()), Moo::new(b.clone())),
                    lex_prefix(lhs_rest, rhs_rest, strict),
                ])),
            ),
        ])),
    )
}

#[cfg(test)]
mod tests {
    use super::*;
    use conjure_cp::ast::{Metadata, Moo, eval_constant};
    use conjure_cp::into_matrix_expr;

    #[test]
    fn scalar_lex_ignores_index_values_and_preserves_prefix_order() {
        use conjure_cp::ast::{AbstractLiteral, Domain, Range};

        let lists: &[&[i32]] = &[&[], &[0], &[1], &[0, 0], &[0, 1], &[1, 0]];
        for lhs in lists {
            for rhs in lists {
                for strict in [false, true] {
                    let operand = |values: &[i32], start: i32| {
                        Moo::new(Expression::AbstractLiteral(
                            Metadata::new(),
                            AbstractLiteral::Matrix(
                                values.iter().copied().map(Expression::from).collect(),
                                Domain::int(vec![Range::Bounded(
                                    start,
                                    start + values.len() as i32 - 1,
                                )]),
                            ),
                        ))
                    };
                    let (left, right) = (operand(lhs, 0), operand(rhs, -3));
                    let expr = if strict {
                        Expression::LexLt(Metadata::new(), left, right)
                    } else {
                        Expression::LexLeq(Metadata::new(), left, right)
                    };
                    let expanded = scalar_lex_comparison(&expr, &SymbolTable::new())
                        .unwrap()
                        .new_expression;
                    assert_eq!(
                        eval_constant(&expanded),
                        Some((if strict { lhs < rhs } else { lhs <= rhs }).into()),
                        "{lhs:?} {rhs:?} strict={strict}"
                    );
                }
            }
        }
    }

    #[test]
    fn non_list_lex_accepts_every_terminal_integer_representation() {
        use conjure_cp::ast::{AbstractLiteral, Domain, Range, SATIntEncoding};

        for encoding in [
            SATIntEncoding::Direct,
            SATIntEncoding::Order,
            SATIntEncoding::Log,
            SATIntEncoding::Offset,
            SATIntEncoding::Rank(vec![(1, 3)]),
        ] {
            let value = Expression::SATInt(
                Metadata::new(),
                encoding,
                Moo::new(into_matrix_expr!(vec![true.into(), false.into()])),
                (1, 3),
            );
            let operand = Moo::new(Expression::AbstractLiteral(
                Metadata::new(),
                AbstractLiteral::Matrix(
                    vec![value.clone(), value],
                    Domain::int(vec![Range::Single(0), Range::Single(2)]),
                ),
            ));
            for strict in [false, true] {
                let expr = if strict {
                    Expression::LexLt(Metadata::new(), operand.clone(), operand.clone())
                } else {
                    Expression::LexLeq(Metadata::new(), operand.clone(), operand.clone())
                };
                let expanded = scalar_lex_comparison(&expr, &SymbolTable::new()).unwrap();
                assert!(matches!(expanded.new_expression, Expression::Or(..)));
            }
        }
    }

    #[test]
    fn scalar_lex_matches_sequence_order_for_empty_prefixes_and_first_differences() {
        let lists: &[&[i32]] = &[&[], &[0], &[1], &[0, 0], &[0, 1], &[1, 0]];
        for lhs in lists {
            for rhs in lists {
                for strict in [false, true] {
                    let left = Moo::new(into_matrix_expr!(
                        lhs.iter()
                            .copied()
                            .map(Expression::from)
                            .collect::<Vec<_>>()
                    ));
                    let right = Moo::new(into_matrix_expr!(
                        rhs.iter()
                            .copied()
                            .map(Expression::from)
                            .collect::<Vec<_>>()
                    ));
                    let expr = if strict {
                        Expression::LexLt(Metadata::new(), left, right)
                    } else {
                        Expression::LexLeq(Metadata::new(), left, right)
                    };
                    let expanded = scalar_lex_comparison(&expr, &SymbolTable::new())
                        .unwrap()
                        .new_expression;
                    assert_eq!(
                        eval_constant(&expanded),
                        Some((if strict { lhs < rhs } else { lhs <= rhs }).into()),
                        "{lhs:?} {rhs:?} strict={strict}"
                    );
                }
            }
        }
    }

    #[test]
    fn compound_lex_prefix_matches_sequence_order() {
        let lists: &[&[i32]] = &[&[], &[0], &[1], &[0, 0], &[0, 1], &[1, 0], &[1, 1, 0]];
        let entries = |list: &[i32]| {
            list.iter()
                .copied()
                .map(Expression::from)
                .collect::<Vec<_>>()
        };
        for lhs in lists {
            for rhs in lists {
                for strict in [false, true] {
                    let decomposed = lex_prefix(&entries(lhs), &entries(rhs), strict);
                    assert_eq!(
                        eval_constant(&decomposed),
                        Some((if strict { lhs < rhs } else { lhs <= rhs }).into()),
                        "{lhs:?} {rhs:?} strict={strict}"
                    );
                }
            }
        }
    }

    #[test]
    fn scalar_lex_waits_for_compound_entry_representations() {
        let tuple = Expression::from(conjure_cp::ast::Literal::AbstractLiteral(
            conjure_cp::ast::AbstractLiteral::Tuple(vec![1.into(), 2.into()]),
        ));
        let matrix = Moo::new(into_matrix_expr!(vec![tuple]));
        for expr in [
            Expression::LexLt(Metadata::new(), matrix.clone(), matrix.clone()),
            Expression::LexLeq(Metadata::new(), matrix.clone(), matrix.clone()),
        ] {
            assert!(scalar_lex_comparison(&expr, &SymbolTable::new()).is_err());
        }
    }
}
