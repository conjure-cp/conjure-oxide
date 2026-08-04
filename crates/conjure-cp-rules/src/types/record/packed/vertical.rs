use super::RecordPacked;
use crate::guard;
use crate::shared::utils::{
    as_cmp_or_lex_op, as_eq_or_neq, collect_cmp_exprs, collect_eq_or_neq, eq_or_neq,
};
use crate::types::record::RecordComponents;
use conjure_cp::ast::{
    AbstractLiteral, Atom, Expression as Expr, Literal, Metadata, Moo, Name, Reference,
    SymbolTable, eval_constant, records::Field,
};
use conjure_cp::representation::ReprRule;
use conjure_cp::rule_engine::ApplicationError::{self, RuleNotApplicable};
use conjure_cp::rule_engine::{
    ApplicationResult, RuleEffect as Reduction, register_rule, register_rule_set,
};
use conjure_cp::{essence_expr, into_matrix_expr};
use parking_lot::MappedRwLockReadGuard;

register_rule_set!("ReprRecordPacked", ("Base"), |_| true);

/// Select a named field (including a nested record-field path) directly from a packed record.
#[register_rule("ReprRecordPacked", 9700, [RecordField])]
fn record_packed_field(expr: &Expr, _: &SymbolTable) -> ApplicationResult {
    let Some((reference, path)) = record_field_path(expr) else {
        return Err(RuleNotApplicable);
    };
    let repr = reference
        .get_repr_as::<RecordPacked>()
        .ok_or(RuleNotApplicable)?;
    let (index, values) = projected_record_field_values(&repr, &path)?;
    Ok(Reduction::pure(unpack_values(&repr, index, &values)))
}

/// Project tuple indices through a named packed-record field without materialising tuple literals.
#[register_rule("ReprRecordPacked", 9750, [SafeIndex])]
fn record_packed_field_tuple_index(expr: &Expr, _: &SymbolTable) -> ApplicationResult {
    guard!(
        let Expr::SafeIndex(_, subject, tuple_indices) = expr &&
        let Some((reference, path)) = record_field_path(subject)
        else {
            return Err(RuleNotApplicable);
        }
    );
    let repr = reference
        .get_repr_as::<RecordPacked>()
        .ok_or(RuleNotApplicable)?;
    let (index, values) = projected_record_field_values(&repr, &path)?;
    let values = project_tuple_values(&values, tuple_indices).ok_or(RuleNotApplicable)?;
    Ok(Reduction::pure(unpack_values(&repr, index, &values)))
}

/// Compare two packed records. Identical layouts compare their ranks directly; differing layouts
/// compare their decoded fields.
#[register_rule("ReprRecordPacked", 9700, [Eq, Neq])]
fn record_packed_var_eq_var(expr: &Expr, _: &SymbolTable) -> ApplicationResult {
    let (lhs, rhs, neq) = as_eq_or_neq(expr)?;
    guard!(
        let Expr::Atomic(_, Atom::Reference(lhs_reference)) = lhs &&
        let Some(lhs_repr) = lhs_reference.get_repr_as::<RecordPacked>() &&
        let Expr::Atomic(_, Atom::Reference(rhs_reference)) = rhs &&
        let Some(rhs_repr) = rhs_reference.get_repr_as::<RecordPacked>()
        else {
            return Err(RuleNotApplicable);
        }
    );
    if lhs_repr.indices != rhs_repr.indices {
        return Err(RuleNotApplicable);
    }

    let new_expression = if lhs_repr.values == rhs_repr.values {
        eq_or_neq(neq, lhs_repr.packed_expr(), rhs_repr.packed_expr())
    } else {
        collect_eq_or_neq(
            neq,
            (0..lhs_repr.values.len()).map(|index| {
                (
                    unpack_entry(&lhs_repr, index),
                    unpack_entry(&rhs_repr, index),
                )
            }),
        )
    };
    Ok(Reduction::pure(new_expression))
}

/// Compare a packed record with a constant record literal.
#[register_rule("ReprRecordPacked", 9700, [Eq, Neq])]
fn record_packed_var_eq_lit(expr: &Expr, _: &SymbolTable) -> ApplicationResult {
    let (lhs, rhs, neq) = as_eq_or_neq(expr)?;
    for (candidate, literal) in [(lhs, rhs), (rhs, lhs)] {
        let Expr::Atomic(_, Atom::Reference(reference)) = candidate else {
            continue;
        };
        let Some(repr) = reference.get_repr_as::<RecordPacked>() else {
            continue;
        };
        let Some(fields) = ordered_record_literals(&repr, literal) else {
            continue;
        };
        let packed = repr.encode(&fields).ok_or(RuleNotApplicable)?;
        return Ok(Reduction::pure(eq_or_neq(
            neq,
            repr.packed_expr(),
            Expr::from(packed),
        )));
    }
    Err(RuleNotApplicable)
}

/// Apply record symmetry ordering directly to matching packed ranks, decoding fields only when
/// layouts differ.
#[register_rule(
    "ReprRecordPacked",
    9700,
    [Lt, Gt, Leq, Geq, LexLt, LexGt, LexLeq, LexGeq]
)]
fn record_packed_var_cmp_var(expr: &Expr, _: &SymbolTable) -> ApplicationResult {
    guard!(
        let Some((lhs, rhs)) = as_cmp_or_lex_op(expr) &&
        let Some(lhs_reference) = comparison_reference(lhs.as_ref()) &&
        let Some(lhs_repr) = lhs_reference.get_repr_as::<RecordPacked>() &&
        let Some(rhs_reference) = comparison_reference(rhs.as_ref()) &&
        let Some(rhs_repr) = rhs_reference.get_repr_as::<RecordPacked>() &&
        lhs_repr.indices == rhs_repr.indices
        else {
            return Err(RuleNotApplicable);
        }
    );

    let result = if lhs_repr.values == rhs_repr.values {
        packed_cmp(expr, lhs_repr.packed_expr(), rhs_repr.packed_expr())
    } else {
        collect_cmp_exprs(
            expr,
            (0..lhs_repr.values.len())
                .map(|index| unpack_entry(&lhs_repr, index))
                .collect(),
            (0..rhs_repr.values.len())
                .map(|index| unpack_entry(&rhs_repr, index))
                .collect(),
        )
    };
    Ok(Reduction::pure(result))
}

/// Apply record symmetry ordering between a packed variable and a literal.
#[register_rule(
    "ReprRecordPacked",
    9700,
    [Lt, Gt, Leq, Geq, LexLt, LexGt, LexLeq, LexGeq]
)]
fn record_packed_var_cmp_lit(expr: &Expr, _: &SymbolTable) -> ApplicationResult {
    guard!(
        let Some((lhs, rhs)) = as_cmp_or_lex_op(expr) &&
        let Some(lhs_reference) = comparison_reference(lhs.as_ref()) &&
        let Some(repr) = lhs_reference.get_repr_as::<RecordPacked>() &&
        let Some(fields) = ordered_record_literals(&repr, rhs.as_ref())
        else {
            return Err(RuleNotApplicable);
        }
    );
    let packed = repr.encode(&fields).ok_or(RuleNotApplicable)?;
    Ok(Reduction::pure(packed_cmp(
        expr,
        repr.packed_expr(),
        Expr::from(packed),
    )))
}

/// Compare a packed field path with any constant literal using matching packed digits. This also
/// handles record- and tuple-valued fields without introducing Minion elements over abstract
/// literals.
#[register_rule("ReprRecordPacked", 9800, [Eq, Neq])]
fn record_packed_field_eq_lit(expr: &Expr, _: &SymbolTable) -> ApplicationResult {
    let (lhs, rhs, neq) = as_eq_or_neq(expr)?;
    for (field_expr, literal_expr) in [(lhs, rhs), (rhs, lhs)] {
        let Some((reference, path)) = record_field_path(field_expr) else {
            continue;
        };
        let Some(repr) = reference.get_repr_as::<RecordPacked>() else {
            continue;
        };
        let Some(literal) = eval_constant(literal_expr) else {
            continue;
        };
        let (index, values) = projected_record_field_values(&repr, &path)?;
        let matching_digits = values
            .iter()
            .enumerate()
            .filter_map(|(digit, value)| value.essence_cmp(&literal).is_eq().then_some(digit))
            .collect::<Vec<_>>();
        let digit = digit_expr(&repr, index);
        let comparisons = matching_digits
            .into_iter()
            .map(|matching| {
                let matching = matching as i32;
                if neq {
                    essence_expr!(&digit != &matching)
                } else {
                    essence_expr!(&digit = &matching)
                }
            })
            .collect::<Vec<_>>();
        let result = if neq {
            Expr::And(Metadata::new(), Moo::new(into_matrix_expr!(comparisons)))
        } else {
            Expr::Or(Metadata::new(), Moo::new(into_matrix_expr!(comparisons)))
        };
        return Ok(Reduction::pure(result));
    }
    Err(RuleNotApplicable)
}

/// Channel direct record components to the corresponding packed digits.
#[register_rule("ReprRecordPacked", 9700, [Eq, Neq])]
fn record_channel_components_packed(expr: &Expr, _: &SymbolTable) -> ApplicationResult {
    let (lhs, rhs, neq) = as_eq_or_neq(expr)?;
    guard!(
        let Expr::Atomic(_, Atom::Reference(lhs_reference)) = lhs &&
        let Expr::Atomic(_, Atom::Reference(rhs_reference)) = rhs
        else {
            return Err(RuleNotApplicable);
        }
    );
    let Some((packed, components)) = as_channel_pair(lhs_reference, rhs_reference) else {
        return Err(RuleNotApplicable);
    };
    if packed.indices != components.indices {
        return Err(RuleNotApplicable);
    }

    Ok(Reduction::pure(collect_eq_or_neq(
        neq,
        components
            .field_refs()
            .enumerate()
            .map(|(index, field)| (field, unpack_entry(&packed, index))),
    )))
}

fn record_field_path(expr: &Expr) -> Option<(Reference, Vec<Name>)> {
    let Expr::RecordField(_, subject, field_name) = expr else {
        return None;
    };
    match subject.as_ref() {
        Expr::Atomic(_, Atom::Reference(reference)) => {
            Some((reference.clone(), vec![field_name.clone()]))
        }
        nested @ Expr::RecordField(..) => {
            let (reference, mut path) = record_field_path(nested)?;
            path.push(field_name.clone());
            Some((reference, path))
        }
        _ => None,
    }
}

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

fn packed_cmp(operator: &Expr, lhs: Expr, rhs: Expr) -> Expr {
    let (lhs, rhs) = (Moo::new(lhs), Moo::new(rhs));
    match operator {
        Expr::Lt(..) | Expr::LexLt(..) => Expr::Lt(Metadata::new(), lhs, rhs),
        Expr::Leq(..) | Expr::LexLeq(..) => Expr::Leq(Metadata::new(), lhs, rhs),
        Expr::Gt(..) | Expr::LexGt(..) => Expr::Gt(Metadata::new(), lhs, rhs),
        Expr::Geq(..) | Expr::LexGeq(..) => Expr::Geq(Metadata::new(), lhs, rhs),
        _ => unreachable!("packed record comparison requires an ordering operator"),
    }
}

fn projected_record_field_values(
    repr: &PackedState<'_>,
    path: &[Name],
) -> Result<(usize, Vec<Literal>), ApplicationError> {
    let (first, remaining) = path.split_first().ok_or(RuleNotApplicable)?;
    let index = *repr.indices.get_by_left(first).ok_or(RuleNotApplicable)?;
    let values = repr.values[index]
        .iter()
        .map(|value| {
            remaining
                .iter()
                .try_fold(value.clone(), |value, field_name| {
                    let Literal::AbstractLiteral(AbstractLiteral::Record(fields)) = value else {
                        return None;
                    };
                    fields
                        .into_iter()
                        .find(|field| field.name == *field_name)
                        .map(|field| field.value)
                })
        })
        .collect::<Option<Vec<_>>>()
        .ok_or(RuleNotApplicable)?;
    Ok((index, values))
}

fn project_tuple_values(values: &[Literal], indices: &[Expr]) -> Option<Vec<Literal>> {
    values
        .iter()
        .map(|value| {
            indices.iter().try_fold(value.clone(), |value, index| {
                let Literal::Int(index) = eval_constant(index)? else {
                    return None;
                };
                let Literal::AbstractLiteral(AbstractLiteral::Tuple(fields)) = value else {
                    return None;
                };
                usize::try_from(index - 1)
                    .ok()
                    .and_then(|index| fields.get(index).cloned())
            })
        })
        .collect()
}

fn ordered_record_literals(repr: &PackedState<'_>, expr: &Expr) -> Option<Vec<Literal>> {
    let literal = eval_constant(expr)?;
    let Literal::AbstractLiteral(AbstractLiteral::Record(fields)) = literal else {
        return None;
    };
    if fields.len() != repr.values.len() {
        return None;
    }
    let mut ordered = vec![None; fields.len()];
    for Field { name, value } in fields {
        let index = *repr.indices.get_by_left(&name)?;
        ordered[index] = Some(value);
    }
    ordered.into_iter().collect()
}

fn unpack_entry(repr: &PackedState<'_>, index: usize) -> Expr {
    unpack_values(repr, index, &repr.values[index])
}

fn unpack_values(repr: &PackedState<'_>, index: usize, values: &[Literal]) -> Expr {
    let digit = digit_expr(repr, index);
    if let Some(minimum) = contiguous_int_min(values) {
        return match minimum {
            0 => digit,
            minimum => essence_expr!(&digit + &minimum),
        };
    }
    let values = values.iter().cloned().map(Expr::from).collect::<Vec<_>>();
    Expr::SafeIndex(
        Metadata::new(),
        Moo::new(into_matrix_expr!(values)),
        vec![essence_expr!(&digit + 1)],
    )
}

fn digit_expr(repr: &PackedState<'_>, index: usize) -> Expr {
    let packed = repr.packed_expr();
    let place = repr.places[index];
    let radix = repr.radices[index];
    match (place, radix, index) {
        (_, 1, _) => Expr::from(0),
        (1, _, 0) => packed,
        (_, _, 0) => essence_expr!(&packed / &place),
        (1, radix, _) => essence_expr!(&packed % &radix),
        (_, radix, _) => essence_expr!((&packed / &place) % &radix),
    }
}

fn contiguous_int_min(values: &[Literal]) -> Option<i32> {
    let Literal::Int(minimum) = values.first()? else {
        return None;
    };
    values
        .iter()
        .enumerate()
        .all(|(offset, value)| *value == Literal::Int(*minimum + offset as i32))
        .then_some(*minimum)
}

type PackedState<'a> = MappedRwLockReadGuard<'a, <RecordPacked as ReprRule>::DeclLevel>;
type ComponentsState<'a> = MappedRwLockReadGuard<'a, <RecordComponents as ReprRule>::DeclLevel>;

fn as_channel_pair<'a>(
    lhs: &'a Reference,
    rhs: &'a Reference,
) -> Option<(PackedState<'a>, ComponentsState<'a>)> {
    let packed = match (
        lhs.get_repr_as::<RecordPacked>(),
        rhs.get_repr_as::<RecordPacked>(),
    ) {
        (Some(lhs), None) => lhs,
        (None, Some(rhs)) => rhs,
        _ => return None,
    };
    let components = match (
        lhs.get_repr_as::<RecordComponents>(),
        rhs.get_repr_as::<RecordComponents>(),
    ) {
        (Some(lhs), None) => lhs,
        (None, Some(rhs)) => rhs,
        _ => return None,
    };
    Some((packed, components))
}

#[cfg(test)]
mod tests {
    use super::*;
    use conjure_cp::ast::{DeclarationPtr, Domain, Name, Range, records::Field};

    #[test]
    fn components_and_packed_channel_field_by_field() {
        let domain = Domain::record(vec![
            Field {
                name: Name::user("flag"),
                value: Domain::bool(),
            },
            Field {
                name: Name::user("value"),
                value: Domain::int(vec![Range::Bounded(1, 3)]),
            },
        ]);
        let mut component_declaration =
            DeclarationPtr::new_find(Name::user("components"), domain.clone());
        let mut packed_declaration = DeclarationPtr::new_find(Name::user("packed"), domain);
        let (component_symbols, _) =
            RecordComponents::init_for(&mut component_declaration).unwrap();
        let (packed_symbols, _) = RecordPacked::init_for(&mut packed_declaration).unwrap();
        let mut symbols = SymbolTable::new();
        symbols.update_insert(component_declaration.clone());
        symbols.update_insert(packed_declaration.clone());
        symbols.extend(component_symbols);
        symbols.extend(packed_symbols);

        let mut component_reference = Reference::new(component_declaration);
        let _ = component_reference
            .select_repr::<RecordComponents>()
            .unwrap();
        let mut packed_reference = Reference::new(packed_declaration);
        let _ = packed_reference.select_repr::<RecordPacked>().unwrap();
        let equality = Expr::Eq(
            Metadata::new(),
            Moo::new(component_reference.into()),
            Moo::new(packed_reference.into()),
        );

        let result = record_channel_components_packed(&equality, &symbols).unwrap();
        let Expr::And(_, constraints) = result.new_expression else {
            panic!("expected field-wise channelling conjunction");
        };
        assert_eq!(constraints.unwrap_list().unwrap().len(), 2);
    }
}
