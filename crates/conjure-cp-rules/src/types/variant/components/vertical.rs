use super::VariantComponents;
use crate::guard;
use crate::shared::utils::{
    as_cmp_or_lex_op, as_eq_or_neq, collect_cmp_exprs, collect_eq_or_neq, eq_or_neq,
};
use conjure_cp::ast::{
    AbstractLiteral, Atom, Expression as Expr, Metadata, Moo, Reference, SymbolTable,
};
use conjure_cp::rule_engine::{
    ApplicationError::RuleNotApplicable, ApplicationResult, RuleEffect as Reduction, register_rule,
};
use conjure_cp::{essence_expr, matrix_expr};

/// Test the active alternative using the component tag.
#[register_rule("ReprGeneral", 9800, [Active])]
fn variant_components_active(expr: &Expr, _: &SymbolTable) -> ApplicationResult {
    guard!(
        let Expr::Active(_, subject, name) = expr &&
        let Expr::Atomic(_, Atom::Reference(reference)) = subject.as_ref() &&
        let Some(repr) = reference.get_repr_as::<VariantComponents>() &&
        let Some(index) = repr.indices.get_by_left(name)
        else {
            return Err(RuleNotApplicable);
        }
    );
    let tag = i32::try_from(*index + 1).map_err(|_| RuleNotApplicable)?;
    let tag_expr = repr.tag_expr();
    Ok(Reduction::pure(essence_expr!(&tag_expr = &tag)))
}

/// Compare an accessed alternative only when that alternative is active.
#[register_rule("ReprGeneral", 9900, [Eq, Neq])]
fn variant_components_field_eq(expr: &Expr, _: &SymbolTable) -> ApplicationResult {
    let (lhs, rhs, neq) = as_eq_or_neq(expr)?;
    for (field_expr, other) in [(lhs, rhs), (rhs, lhs)] {
        let Some((repr, index)) = component_field(field_expr) else {
            continue;
        };
        let tag = i32::try_from(index + 1).map_err(|_| RuleNotApplicable)?;
        let tag_expr = repr.tag_expr();
        let active = essence_expr!(&tag_expr = &tag);
        let field = Reference::new(repr.fields[index].clone());
        let comparison = eq_or_neq(neq, field.into(), other.clone());
        return Ok(Reduction::pure(Expr::And(
            Metadata::new(),
            Moo::new(matrix_expr![active, comparison]),
        )));
    }
    Err(RuleNotApplicable)
}

/// Project an alternative through a one-element unsafe index so inactivity remains undefined.
#[register_rule("ReprGeneral", 9400, [RecordField])]
fn variant_components_field(expr: &Expr, _: &SymbolTable) -> ApplicationResult {
    let Some((repr, index)) = component_field(expr) else {
        return Err(RuleNotApplicable);
    };
    let tag = i32::try_from(index + 1).map_err(|_| RuleNotApplicable)?;
    let tag_expr = repr.tag_expr();
    let active = essence_expr!(&tag_expr = &tag);
    Ok(Reduction::pure(Expr::UnsafeIndex(
        Metadata::new(),
        Moo::new(matrix_expr![Expr::from(Reference::new(
            repr.fields[index].clone()
        ))]),
        vec![Expr::ToInt(Metadata::new(), Moo::new(active))],
    )))
}

/// Compare two component variants using their canonical tag-and-fields layouts.
#[register_rule("ReprGeneral", 9700, [Eq, Neq])]
fn variant_components_var_eq_var(expr: &Expr, _: &SymbolTable) -> ApplicationResult {
    let (lhs, rhs, neq) = as_eq_or_neq(expr)?;
    guard!(
        let Expr::Atomic(_, Atom::Reference(lhs_reference)) = lhs &&
        let Some(lhs_repr) = lhs_reference.get_repr_as::<VariantComponents>() &&
        let Expr::Atomic(_, Atom::Reference(rhs_reference)) = rhs &&
        let Some(rhs_repr) = rhs_reference.get_repr_as::<VariantComponents>() &&
        lhs_repr.indices == rhs_repr.indices
        else {
            return Err(RuleNotApplicable);
        }
    );
    let pairs = std::iter::once((lhs_repr.tag_expr(), rhs_repr.tag_expr())).chain(
        lhs_repr
            .field_refs()
            .zip(rhs_repr.field_refs())
            .map(|(lhs, rhs)| (lhs.into(), rhs.into())),
    );
    Ok(Reduction::pure(collect_eq_or_neq(neq, pairs)))
}

/// Compare a component variant with a variant literal.
#[register_rule("ReprGeneral", 9700, [Eq, Neq])]
fn variant_components_var_eq_lit(expr: &Expr, _: &SymbolTable) -> ApplicationResult {
    let (lhs, rhs, neq) = as_eq_or_neq(expr)?;
    for (candidate, literal) in [(lhs, rhs), (rhs, lhs)] {
        let Expr::Atomic(_, Atom::Reference(reference)) = candidate else {
            continue;
        };
        let Some(repr) = reference.get_repr_as::<VariantComponents>() else {
            continue;
        };
        let Some((name, value)) = variant_literal(literal) else {
            continue;
        };
        let Some(index) = repr.indices.get_by_left(&name).copied() else {
            continue;
        };
        let tag = i32::try_from(index + 1).map_err(|_| RuleNotApplicable)?;
        let tag_comparison = eq_or_neq(neq, repr.tag_expr(), Expr::from(tag));
        let field_comparison = eq_or_neq(
            neq,
            Reference::new(repr.fields[index].clone()).into(),
            value,
        );
        let result = if neq {
            Expr::Or(
                Metadata::new(),
                Moo::new(matrix_expr![tag_comparison, field_comparison]),
            )
        } else {
            Expr::And(
                Metadata::new(),
                Moo::new(matrix_expr![tag_comparison, field_comparison]),
            )
        };
        return Ok(Reduction::pure(result));
    }
    Err(RuleNotApplicable)
}

/// Apply Conjure's tag-then-components symmetry ordering.
#[register_rule(
    "ReprGeneral",
    9700,
    [Lt, Gt, Leq, Geq, LexLt, LexGt, LexLeq, LexGeq]
)]
fn variant_components_cmp(expr: &Expr, _: &SymbolTable) -> ApplicationResult {
    guard!(
        let Some((lhs, rhs)) = as_cmp_or_lex_op(expr) &&
        let Some(lhs_reference) = comparison_reference(lhs.as_ref()) &&
        let Some(lhs_repr) = lhs_reference.get_repr_as::<VariantComponents>() &&
        let Some(rhs_reference) = comparison_reference(rhs.as_ref()) &&
        let Some(rhs_repr) = rhs_reference.get_repr_as::<VariantComponents>() &&
        lhs_repr.indices == rhs_repr.indices
        else {
            return Err(RuleNotApplicable);
        }
    );
    let lhs = std::iter::once(lhs_repr.tag_expr())
        .chain(lhs_repr.field_refs().map(Into::into))
        .collect();
    let rhs = std::iter::once(rhs_repr.tag_expr())
        .chain(rhs_repr.field_refs().map(Into::into))
        .collect();
    Ok(Reduction::pure(collect_cmp_exprs(expr, lhs, rhs)))
}

fn component_field(
    expr: &Expr,
) -> Option<(
    parking_lot::MappedRwLockReadGuard<
        '_,
        <VariantComponents as conjure_cp::representation::ReprRule>::DeclLevel,
    >,
    usize,
)> {
    let Expr::RecordField(_, subject, name) = expr else {
        return None;
    };
    let Expr::Atomic(_, Atom::Reference(reference)) = subject.as_ref() else {
        return None;
    };
    let repr = reference.get_repr_as::<VariantComponents>()?;
    let index = *repr.indices.get_by_left(name)?;
    Some((repr, index))
}

fn variant_literal(expr: &Expr) -> Option<(conjure_cp::ast::Name, Expr)> {
    match expr {
        Expr::AbstractLiteral(_, AbstractLiteral::Variant(field)) => {
            Some((field.name.clone(), field.value.clone()))
        }
        Expr::Atomic(
            _,
            Atom::Literal(conjure_cp::ast::Literal::AbstractLiteral(AbstractLiteral::Variant(
                field,
            ))),
        ) => Some((field.name.clone(), Expr::from(field.value.clone()))),
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
