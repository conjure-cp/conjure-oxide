use super::VariantPacked;
use crate::guard;
use crate::shared::utils::{as_cmp_or_lex_op, as_eq_or_neq, eq_or_neq};
use crate::types::variant::VariantComponents;
use conjure_cp::ast::{
    AbstractLiteral, Atom, Expression as Expr, Literal, Metadata, Moo, Name, Reference, SymbolTable,
};
use conjure_cp::representation::ReprRule;
use conjure_cp::rule_engine::{
    ApplicationError::RuleNotApplicable, ApplicationResult, RuleEffect as Reduction, register_rule,
    register_rule_set,
};
use conjure_cp::{essence_expr, into_matrix_expr, matrix_expr};
use parking_lot::MappedRwLockReadGuard;

register_rule_set!("ReprVariantPacked", ("Base"), |_| true);

/// Test whether a packed rank lies in an alternative's interval.
#[register_rule("ReprVariantPacked", 9800, [Active])]
fn variant_packed_active(expr: &Expr, _: &SymbolTable) -> ApplicationResult {
    guard!(
        let Expr::Active(_, subject, name) = expr &&
        let Expr::Atomic(_, Atom::Reference(reference)) = subject.as_ref() &&
        let Some(repr) = reference.get_repr_as::<VariantPacked>() &&
        let Some(index) = repr.indices.get_by_left(name).copied()
        else {
            return Err(RuleNotApplicable);
        }
    );
    Ok(Reduction::pure(active_expr(&repr, index)))
}

/// Compare an accessed packed alternative only inside its active interval.
#[register_rule("ReprVariantPacked", 9900, [Eq, Neq])]
fn variant_packed_field_eq(expr: &Expr, _: &SymbolTable) -> ApplicationResult {
    let (lhs, rhs, neq) = as_eq_or_neq(expr)?;
    for (field_expr, other) in [(lhs, rhs), (rhs, lhs)] {
        let Some((repr, index)) = packed_field(field_expr) else {
            continue;
        };
        let alternatives = repr.values[index]
            .iter()
            .enumerate()
            .map(|(digit, value)| {
                let rank = repr.offsets[index] + i32::try_from(digit).unwrap();
                let packed = repr.packed_expr();
                Expr::And(
                    Metadata::new(),
                    Moo::new(matrix_expr![
                        essence_expr!(&packed = &rank),
                        eq_or_neq(neq, other.clone(), Expr::from(value.clone())),
                    ]),
                )
            })
            .collect::<Vec<_>>();
        return Ok(Reduction::pure(Expr::Or(
            Metadata::new(),
            Moo::new(into_matrix_expr!(alternatives)),
        )));
    }
    Err(RuleNotApplicable)
}

/// Project a packed alternative with undefined out-of-interval indexing.
#[register_rule("ReprVariantPacked", 9400, [RecordField])]
fn variant_packed_field(expr: &Expr, _: &SymbolTable) -> ApplicationResult {
    let Some((repr, index)) = packed_field(expr) else {
        return Err(RuleNotApplicable);
    };
    Ok(Reduction::pure(unpack_field(&repr, index)))
}

/// Matching packed layouts compare their canonical ranks directly.
#[register_rule("ReprVariantPacked", 9700, [Eq, Neq])]
fn variant_packed_var_eq_var(expr: &Expr, _: &SymbolTable) -> ApplicationResult {
    let (lhs, rhs, neq) = as_eq_or_neq(expr)?;
    guard!(
        let Expr::Atomic(_, Atom::Reference(lhs_reference)) = lhs &&
        let Some(lhs_repr) = lhs_reference.get_repr_as::<VariantPacked>() &&
        let Expr::Atomic(_, Atom::Reference(rhs_reference)) = rhs &&
        let Some(rhs_repr) = rhs_reference.get_repr_as::<VariantPacked>() &&
        lhs_repr.indices == rhs_repr.indices &&
        lhs_repr.values == rhs_repr.values
        else {
            return Err(RuleNotApplicable);
        }
    );
    Ok(Reduction::pure(eq_or_neq(
        neq,
        lhs_repr.packed_expr(),
        rhs_repr.packed_expr(),
    )))
}

/// Encode a constant variant directly as its packed rank.
#[register_rule("ReprVariantPacked", 9700, [Eq, Neq])]
fn variant_packed_var_eq_lit(expr: &Expr, _: &SymbolTable) -> ApplicationResult {
    let (lhs, rhs, neq) = as_eq_or_neq(expr)?;
    for (candidate, literal) in [(lhs, rhs), (rhs, lhs)] {
        let Expr::Atomic(_, Atom::Reference(reference)) = candidate else {
            continue;
        };
        let Some(repr) = reference.get_repr_as::<VariantPacked>() else {
            continue;
        };
        let Some((name, value)) = variant_literal(literal) else {
            continue;
        };
        let index = *repr.indices.get_by_left(&name).ok_or(RuleNotApplicable)?;
        let alternatives = repr.values[index]
            .iter()
            .enumerate()
            .map(|(digit, field_value)| {
                let rank = repr.offsets[index] + i32::try_from(digit).unwrap();
                let packed = repr.packed_expr();
                Expr::And(
                    Metadata::new(),
                    Moo::new(matrix_expr![
                        essence_expr!(&packed = &rank),
                        essence_expr!(&value = &field_value),
                    ]),
                )
            })
            .collect::<Vec<_>>();
        let equality = Expr::Or(Metadata::new(), Moo::new(into_matrix_expr!(alternatives)));
        return Ok(Reduction::pure(if neq {
            Expr::Not(Metadata::new(), Moo::new(equality))
        } else {
            equality
        }));
    }
    Err(RuleNotApplicable)
}

/// Packed ranks already follow the variant symmetry order.
#[register_rule(
    "ReprVariantPacked",
    9700,
    [Lt, Gt, Leq, Geq, LexLt, LexGt, LexLeq, LexGeq]
)]
fn variant_packed_cmp(expr: &Expr, _: &SymbolTable) -> ApplicationResult {
    guard!(
        let Some((lhs, rhs)) = as_cmp_or_lex_op(expr) &&
        let Some(lhs_reference) = comparison_reference(lhs.as_ref()) &&
        let Some(lhs_repr) = lhs_reference.get_repr_as::<VariantPacked>() &&
        let Some(rhs_reference) = comparison_reference(rhs.as_ref()) &&
        let Some(rhs_repr) = rhs_reference.get_repr_as::<VariantPacked>() &&
        lhs_repr.indices == rhs_repr.indices &&
        lhs_repr.values == rhs_repr.values
        else {
            return Err(RuleNotApplicable);
        }
    );
    Ok(Reduction::pure(packed_cmp(
        expr,
        lhs_repr.packed_expr(),
        rhs_repr.packed_expr(),
    )))
}

/// Channel components and packed layouts by enumerating the finite disjoint union.
#[register_rule("ReprVariantPacked", 9700, [Eq, Neq])]
fn variant_channel_components_packed(expr: &Expr, _: &SymbolTable) -> ApplicationResult {
    let (lhs, rhs, neq) = as_eq_or_neq(expr)?;
    guard!(
        !neq &&
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

    let alternatives = packed
        .values
        .iter()
        .enumerate()
        .flat_map(|(index, values)| {
            values.iter().enumerate().map({
                let packed = &packed;
                let components = &components;
                move |(digit, value)| {
                    let rank = packed.offsets[index] + i32::try_from(digit).unwrap();
                    let tag = i32::try_from(index + 1).unwrap();
                    let packed_expr = packed.packed_expr();
                    let tag_expr = components.tag_expr();
                    let field = Reference::new(components.fields[index].clone());
                    Expr::And(
                        Metadata::new(),
                        Moo::new(matrix_expr![
                            essence_expr!(&packed_expr = &rank),
                            essence_expr!(&tag_expr = &tag),
                            essence_expr!(&field = &value),
                        ]),
                    )
                }
            })
        })
        .collect::<Vec<_>>();
    Ok(Reduction::pure(Expr::Or(
        Metadata::new(),
        Moo::new(into_matrix_expr!(alternatives)),
    )))
}

fn active_expr(repr: &PackedState<'_>, index: usize) -> Expr {
    let lower = repr.offsets[index];
    let upper = lower + i32::try_from(repr.values[index].len()).unwrap();
    let packed = repr.packed_expr();
    Expr::And(
        Metadata::new(),
        Moo::new(matrix_expr![
            essence_expr!(&packed >= &lower),
            essence_expr!(&packed < &upper),
        ]),
    )
}

fn unpack_field(repr: &PackedState<'_>, index: usize) -> Expr {
    let values = repr.values[index]
        .iter()
        .cloned()
        .map(Expr::from)
        .collect::<Vec<_>>();
    let offset = repr.offsets[index];
    let packed = repr.packed_expr();
    Expr::UnsafeIndex(
        Metadata::new(),
        Moo::new(into_matrix_expr!(values)),
        vec![essence_expr!(&packed - &offset + 1)],
    )
}

fn packed_field(expr: &Expr) -> Option<(PackedState<'_>, usize)> {
    let Expr::RecordField(_, subject, name) = expr else {
        return None;
    };
    let Expr::Atomic(_, Atom::Reference(reference)) = subject.as_ref() else {
        return None;
    };
    let repr = reference.get_repr_as::<VariantPacked>()?;
    let index = *repr.indices.get_by_left(name)?;
    Some((repr, index))
}

fn variant_literal(expr: &Expr) -> Option<(Name, Expr)> {
    match expr {
        Expr::AbstractLiteral(_, AbstractLiteral::Variant(field)) => {
            Some((field.name.clone(), field.value.clone()))
        }
        Expr::Atomic(
            _,
            Atom::Literal(Literal::AbstractLiteral(AbstractLiteral::Variant(field))),
        ) => Some((field.name.clone(), Expr::from(field.value.clone()))),
        _ => None,
    }
}

fn packed_cmp(operator: &Expr, lhs: Expr, rhs: Expr) -> Expr {
    let (lhs, rhs) = (Moo::new(lhs), Moo::new(rhs));
    match operator {
        Expr::Lt(..) | Expr::LexLt(..) => Expr::Lt(Metadata::new(), lhs, rhs),
        Expr::Leq(..) | Expr::LexLeq(..) => Expr::Leq(Metadata::new(), lhs, rhs),
        Expr::Gt(..) | Expr::LexGt(..) => Expr::Gt(Metadata::new(), lhs, rhs),
        Expr::Geq(..) | Expr::LexGeq(..) => Expr::Geq(Metadata::new(), lhs, rhs),
        _ => unreachable!("packed variant comparison requires an ordering operator"),
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

type PackedState<'a> = MappedRwLockReadGuard<'a, <VariantPacked as ReprRule>::DeclLevel>;
type ComponentsState<'a> = MappedRwLockReadGuard<'a, <VariantComponents as ReprRule>::DeclLevel>;

fn as_channel_pair<'a>(
    lhs: &'a Reference,
    rhs: &'a Reference,
) -> Option<(PackedState<'a>, ComponentsState<'a>)> {
    let packed = lhs
        .get_repr_as::<VariantPacked>()
        .or_else(|| rhs.get_repr_as::<VariantPacked>())?;
    let components = lhs
        .get_repr_as::<VariantComponents>()
        .or_else(|| rhs.get_repr_as::<VariantComponents>())?;
    Some((packed, components))
}
