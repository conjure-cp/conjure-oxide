use crate::guard;
use crate::shared::utils::{as_eq_or_neq, collect_eq_or_neq};
use conjure_cp::ast::{AbstractLiteral, Atom, Expression as Expr, Literal, SymbolTable, records::Field};
use conjure_cp::bug_assert;
use conjure_cp::rule_engine::ApplicationError::RuleNotApplicable;
use conjure_cp::rule_engine::{ApplicationResult, RuleEffect as Reduction, register_rule};

/// Gets the fields of a record literal expression, if it is one. Mirrors
/// `types::tuple::horizontal`'s own `tuple_expr_entries`/`tuple_literal_eq_literal` -- see there for
/// why an *inline* literal (as opposed to a reference into some record representation) needs its own
/// rule at all: no representation-specific rule matches two freshly-built literal expressions, since
/// they all key off an atomic reference on at least one side.
fn record_expr_entries(expr: &Expr) -> Option<Vec<Field<Expr>>> {
    match expr {
        Expr::AbstractLiteral(_, AbstractLiteral::Record(fields)) => Some(fields.clone()),
        Expr::Atomic(_, Atom::Literal(Literal::AbstractLiteral(AbstractLiteral::Record(fields)))) => {
            Some(
                fields
                    .iter()
                    .cloned()
                    .map(|Field { name, value }| Field {
                        name,
                        value: Expr::from(value),
                    })
                    .collect(),
            )
        }
        _ => None,
    }
}

/// Compare two inline record literal expressions field by field, matching fields by name (a
/// record's own field order need not match between two independently-built literals, unlike a
/// tuple's positional fields).
/// ```plain
/// {a: 1, b: 2} = {a: 1, b: 2} ~> 1 = 1 /\ 2 = 2
/// ```
#[register_rule("Base", 8700, [Eq, Neq])]
fn record_literal_eq_literal(expr: &Expr, _: &SymbolTable) -> ApplicationResult {
    let (lhs, rhs, neq) = as_eq_or_neq(expr)?;
    guard!(
        let Some(lhs_fields) = record_expr_entries(lhs) &&
        let Some(rhs_fields) = record_expr_entries(rhs)
        else {
            return Err(RuleNotApplicable);
        }
    );
    bug_assert!(
        lhs_fields.len() == rhs_fields.len(),
        "equality on record literals with different shapes!"
    );

    let mut rhs_fields = rhs_fields;
    let mut pairs = Vec::with_capacity(lhs_fields.len());
    for Field { name, value: lhs_value } in lhs_fields {
        let position = rhs_fields
            .iter()
            .position(|field| field.name == name)
            .unwrap_or_else(|| {
                panic!("equality on record literals with mismatched field names: missing `{name}`")
            });
        let rhs_value = rhs_fields.remove(position).value;
        pairs.push((lhs_value, rhs_value));
    }

    let new_expr = collect_eq_or_neq(neq, pairs.into_iter());
    Ok(Reduction::pure(new_expr))
}

/// Index directly into an inline record literal by field name, e.g. one just built by another rule
/// on the fly rather than read from the input model.
/// ```plain
/// {a: 1, b: 2}.b ~> 2
/// ```
#[register_rule("Base", 8700, [RecordField])]
fn record_literal_field(expr: &Expr, _: &SymbolTable) -> ApplicationResult {
    guard!(
        let Expr::RecordField(_, subject, field_name) = expr &&
        let Some(fields) = record_expr_entries(subject)
        else {
            return Err(RuleNotApplicable);
        }
    );

    let field = fields
        .into_iter()
        .find(|field| &field.name == field_name)
        .unwrap_or_else(|| panic!("record literal has no field named `{field_name}`"));
    Ok(Reduction::pure(field.value))
}

#[cfg(test)]
mod tests {
    use super::*;
    use conjure_cp::ast::{Metadata, Moo, Name};
    use conjure_cp::range;
    use conjure_cp::rule_engine::get_rule_by_name;
    use uniplate::Uniplate;

    fn int_lit(n: i32) -> Expr {
        Expr::Atomic(Metadata::new(), Atom::Literal(n.into()))
    }

    fn record_lit(fields: Vec<(&str, Expr)>) -> Expr {
        Expr::AbstractLiteral(
            Metadata::new(),
            AbstractLiteral::Record(
                fields
                    .into_iter()
                    .map(|(name, value)| Field {
                        name: Name::user(name),
                        value,
                    })
                    .collect(),
            ),
        )
    }

    #[test]
    fn record_literal_eq_literal_decomposes_field_by_field() {
        let lhs = record_lit(vec![("fst", int_lit(7)), ("snd", int_lit(13))]);
        let rhs = record_lit(vec![("fst", int_lit(7)), ("snd", int_lit(17))]);
        let expr = Expr::Eq(Metadata::new(), Moo::new(lhs), Moo::new(rhs));

        let rule = get_rule_by_name("record_literal_eq_literal").expect("rule registered");
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

    #[test]
    fn record_literal_eq_literal_matches_fields_by_name_regardless_of_order() {
        let lhs = record_lit(vec![("fst", int_lit(7)), ("snd", int_lit(13))]);
        let rhs = record_lit(vec![("snd", int_lit(13)), ("fst", int_lit(7))]);
        let expr = Expr::Eq(Metadata::new(), Moo::new(lhs), Moo::new(rhs));

        let rule = get_rule_by_name("record_literal_eq_literal").expect("rule registered");
        assert!(rule.apply(&expr, &SymbolTable::new()).is_ok());
    }

    #[test]
    fn record_literal_eq_literal_is_not_applicable_when_a_side_is_a_reference() {
        let lhs = Expr::Atomic(
            Metadata::new(),
            Atom::Reference(conjure_cp::ast::Reference::new(
                SymbolTable::new().gen_find(&conjure_cp::domain_int!(1..3)),
            )),
        );
        let rhs = record_lit(vec![("fst", int_lit(1))]);
        let expr = Expr::Eq(Metadata::new(), Moo::new(lhs), Moo::new(rhs));

        let rule = get_rule_by_name("record_literal_eq_literal").expect("rule registered");
        assert!(rule.apply(&expr, &SymbolTable::new()).is_err());
    }

    #[test]
    fn record_literal_field_selects_the_named_field() {
        let subject = record_lit(vec![("fst", int_lit(7)), ("snd", int_lit(8))]);
        let expr = Expr::RecordField(Metadata::new(), Moo::new(subject), Name::user("snd"));

        let rule = get_rule_by_name("record_literal_field").expect("rule registered");
        let result = rule
            .apply(&expr, &SymbolTable::new())
            .expect("should select the named field");
        assert_eq!(result.new_expression, int_lit(8));
    }
}
