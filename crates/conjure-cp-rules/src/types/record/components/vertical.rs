use super::RecordComponents;
use crate::guard;
use crate::shared::utils::{as_cmp_or_lex_op, as_eq_or_neq, collect_cmp_exprs, collect_eq_or_neq};
use conjure_cp::ast::{
    AbstractLiteral, Atom, Expression as Expr, Literal, Reference, SymbolTable, records::Field,
};
use conjure_cp::rule_engine::{
    ApplicationError::RuleNotApplicable, ApplicationResult, RuleEffect as Reduction, register_rule,
};
use conjure_cp::utils::BiMap;
use itertools::izip;

fn comparison_reference(expr: &Expr) -> Option<Reference> {
    if let Expr::Atomic(_, Atom::Reference(reference)) = expr {
        return Some(reference.clone());
    }
    let values = expr.unwrap_list()?;
    let [Expr::Atomic(_, Atom::Reference(reference))] = values.as_slice() else {
        return None;
    };
    Some(reference.clone())
}

/// Select a named field from the direct component representation.
#[register_rule("ReprGeneral", 9500, [RecordField])]
fn record_components_field(expr: &Expr, _: &SymbolTable) -> ApplicationResult {
    guard!(
        let Expr::RecordField(_, subject, field_name) = expr       &&
        let Expr::Atomic(_, Atom::Reference(reference)) = subject.as_ref() &&
        let Some(repr) = reference.get_repr_as::<RecordComponents>()
        else {
            return Err(RuleNotApplicable);
        }
    );

    let field = repr.field_ref(field_name).ok_or(RuleNotApplicable)?;
    Ok(Reduction::pure(field.into()))
}

/// Compare two component-represented records field by field.
#[register_rule("ReprGeneral", 9400, [Eq, Neq])]
fn record_components_var_eq_var(expr: &Expr, _: &SymbolTable) -> ApplicationResult {
    let (lhs, rhs, neq) = as_eq_or_neq(expr)?;
    guard!(
        let Expr::Atomic(_, Atom::Reference(lhs_reference)) = lhs &&
        let Some(lhs_repr) = lhs_reference.get_repr_as::<RecordComponents>() &&
        let Expr::Atomic(_, Atom::Reference(rhs_reference)) = rhs &&
        let Some(rhs_repr) = rhs_reference.get_repr_as::<RecordComponents>()
        else {
            return Err(RuleNotApplicable);
        }
    );
    if lhs_repr.indices != rhs_repr.indices {
        return Err(RuleNotApplicable);
    }

    Ok(Reduction::pure(collect_eq_or_neq(
        neq,
        izip!(lhs_repr.field_refs(), rhs_repr.field_refs()),
    )))
}

/// Compare a component-represented record with a record literal.
#[register_rule("ReprGeneral", 9400, [Eq, Neq])]
fn record_components_var_eq_lit(expr: &Expr, _: &SymbolTable) -> ApplicationResult {
    let (lhs, rhs, neq) = as_eq_or_neq(expr)?;
    let Some((reference, literal)) =
        represented_reference_and_literal::<RecordComponents>(lhs, rhs)
    else {
        return Err(RuleNotApplicable);
    };
    let repr = reference
        .get_repr_as::<RecordComponents>()
        .ok_or(RuleNotApplicable)?;
    let fields = ordered_record_entries(&repr.indices, repr.fields.len(), &literal)
        .ok_or(RuleNotApplicable)?;

    Ok(Reduction::pure(collect_eq_or_neq(
        neq,
        izip!(repr.field_refs(), fields),
    )))
}

/// Apply the record symmetry ordering to component-represented variables field by field.
#[register_rule(
    "ReprGeneral",
    9400,
    [Lt, Gt, Leq, Geq, LexLt, LexGt, LexLeq, LexGeq]
)]
fn record_components_var_cmp_var(expr: &Expr, _: &SymbolTable) -> ApplicationResult {
    guard!(
        let Some((lhs, rhs)) = as_cmp_or_lex_op(expr) &&
        let Some(lhs_reference) = comparison_reference(lhs.as_ref()) &&
        let Some(lhs_repr) = lhs_reference.get_repr_as::<RecordComponents>() &&
        let Some(rhs_reference) = comparison_reference(rhs.as_ref()) &&
        let Some(rhs_repr) = rhs_reference.get_repr_as::<RecordComponents>() &&
        lhs_repr.indices == rhs_repr.indices
        else {
            return Err(RuleNotApplicable);
        }
    );

    Ok(Reduction::pure(collect_cmp_exprs(
        expr,
        lhs_repr.field_exprs(),
        rhs_repr.field_exprs(),
    )))
}

/// Apply the record symmetry ordering between a component variable and a literal.
#[register_rule(
    "ReprGeneral",
    9400,
    [Lt, Gt, Leq, Geq, LexLt, LexGt, LexLeq, LexGeq]
)]
fn record_components_var_cmp_lit(expr: &Expr, _: &SymbolTable) -> ApplicationResult {
    guard!(
        let Some((lhs, rhs)) = as_cmp_or_lex_op(expr) &&
        let Some(lhs_reference) = comparison_reference(lhs.as_ref()) &&
        let Some(lhs_repr) = lhs_reference.get_repr_as::<RecordComponents>() &&
        let Some(rhs_fields) = ordered_record_entries(
            &lhs_repr.indices,
            lhs_repr.fields.len(),
            rhs.as_ref(),
        )
        else {
            return Err(RuleNotApplicable);
        }
    );

    Ok(Reduction::pure(collect_cmp_exprs(
        expr,
        lhs_repr.field_exprs(),
        rhs_fields,
    )))
}

fn represented_reference_and_literal<R: conjure_cp::representation::ReprRule>(
    lhs: &Expr,
    rhs: &Expr,
) -> Option<(Reference, Expr)> {
    for (candidate, literal) in [(lhs, rhs), (rhs, lhs)] {
        let Expr::Atomic(_, Atom::Reference(reference)) = candidate else {
            continue;
        };
        if reference.get_repr_as::<R>().is_some() && record_entries(literal).is_some() {
            return Some((reference.clone(), literal.clone()));
        }
    }
    None
}

pub(super) fn ordered_record_entries(
    indices: &BiMap<conjure_cp::ast::Name, usize>,
    len: usize,
    expr: &Expr,
) -> Option<Vec<Expr>> {
    let entries = record_entries(expr)?;
    if entries.len() != len {
        return None;
    }
    let mut ordered = vec![None; len];
    for Field { name, value } in entries {
        let index = *indices.get_by_left(&name)?;
        ordered[index] = Some(value);
    }
    ordered.into_iter().collect()
}

fn record_entries(expr: &Expr) -> Option<Vec<Field<Expr>>> {
    match expr {
        Expr::AbstractLiteral(_, AbstractLiteral::Record(entries)) => Some(entries.clone()),
        Expr::Atomic(
            _,
            Atom::Literal(Literal::AbstractLiteral(AbstractLiteral::Record(entries))),
        ) => Some(
            entries
                .iter()
                .cloned()
                .map(|Field { name, value }| Field {
                    name,
                    value: Expr::from(value),
                })
                .collect(),
        ),
        _ => None,
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use conjure_cp::ast::{DeclarationPtr, Domain, HasDomain, Metadata, Moo, Name, Range};
    use conjure_cp::representation::ReprRule;

    fn record_domain() -> conjure_cp::ast::DomainPtr {
        Domain::record(vec![
            Field {
                name: Name::user("b"),
                value: Domain::int(vec![Range::Bounded(0, 2)]),
            },
            Field {
                name: Name::user("a"),
                value: Domain::bool(),
            },
        ])
    }

    #[test]
    fn component_field_access_uses_canonical_name_mapping() {
        let mut declaration = DeclarationPtr::new_find(Name::user("x"), record_domain());
        let (extra, _) = RecordComponents::init_for(&mut declaration).unwrap();
        let mut symbols = SymbolTable::new();
        symbols.update_insert(declaration.clone());
        symbols.extend(extra);

        let mut reference = Reference::new(declaration);
        let _ = reference.select_repr::<RecordComponents>().unwrap();
        let expression =
            Expr::RecordField(Metadata::new(), Moo::new(reference.into()), Name::user("a"));
        let result = record_components_field(&expression, &symbols).unwrap();
        let Expr::Atomic(_, Atom::Reference(selected)) = result.new_expression else {
            panic!("expected a component reference");
        };
        assert_eq!(selected.domain_of(), Domain::bool());
    }
}
