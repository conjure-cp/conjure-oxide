use super::MatrixPacked;
use conjure_cp::ast::{
    Atom, DeclarationKind, Expression as Expr, Metadata, Moo, Reference, SymbolTable,
};
use conjure_cp::representation::ReprRule;
use conjure_cp::rule_engine::{
    ApplicationError::RuleNotApplicable, ApplicationResult, RuleEffect, register_rule,
    register_rule_set,
};
use std::cell::Cell;
use uniplate::Biplate;

// Whole-matrix mixed-radix decoding can be expensive for indexing-heavy models. Keep it explicit
// until the representation heuristic can compare it with MatrixComponents at the same choice site.
register_rule_set!("ReprMatrixPacked", ("Base"), |_| false);

fn decl_may_need_packed_matrix_init(decl: &conjure_cp::ast::DeclarationPtr) -> bool {
    matches!(
        &decl.kind() as &DeclarationKind,
        DeclarationKind::Find(..)
            | DeclarationKind::FindAuxiliary(..)
            | DeclarationKind::ValueLetting(..)
    ) && decl.reprs().is_empty()
}

fn select_packed_matrix_references(expr: &Expr, changed: &Cell<bool>) -> Expr {
    expr.descend_bi(&|mut reference: Reference| {
        if reference.repr.is_none()
            && reference.ptr.reprs().has_repr(MatrixPacked::STORED)
            && reference.select_repr_via(&MatrixPacked).is_ok()
        {
            changed.set(true);
        }
        reference
    })
}

/// Prefer the whole-matrix packed layout before the legacy components fallback.
/// Composite element domains remain inapplicable here and continue through MatrixComponents.
#[register_rule("ReprMatrixPacked", 8501, [Root])]
fn select_packed_matrices(expr: &Expr, symtab: &SymbolTable) -> ApplicationResult {
    let Expr::Root(..) = expr else {
        return Err(RuleNotApplicable);
    };
    if !symtab
        .iter_local()
        .any(|(_, declaration)| decl_may_need_packed_matrix_init(declaration))
    {
        return Err(RuleNotApplicable);
    }

    let mut symbols = symtab.clone();
    let mut constraints = Vec::new();
    let changed = Cell::new(false);
    for (_, declaration) in symtab.iter_local() {
        if !decl_may_need_packed_matrix_init(declaration) {
            continue;
        }
        let mut declaration = declaration.clone();
        let Ok((new_symbols, new_constraints)) = MatrixPacked::init_for(&mut declaration) else {
            continue;
        };
        symbols.update_insert(declaration);
        symbols.extend(new_symbols);
        constraints.extend(new_constraints);
        changed.set(true);
    }

    if !changed.get() {
        return Err(RuleNotApplicable);
    }
    let expression = select_packed_matrix_references(expr, &changed);
    Ok(RuleEffect::new(expression, constraints, symbols))
}

/// A packed matrix is semantically transparent: decode it to an ordinary indexed matrix
/// expression, after which the representation-independent matrix rules apply.
#[register_rule("ReprMatrixPacked", 9800, [Atomic / Reference])]
fn decode_packed_matrix_reference(expr: &Expr, _: &SymbolTable) -> ApplicationResult {
    let Expr::Atomic(_, Atom::Reference(reference)) = expr else {
        return Err(RuleNotApplicable);
    };
    let Some(representation) = reference.get_repr_as::<MatrixPacked>() else {
        return Err(RuleNotApplicable);
    };
    Ok(RuleEffect::pure(representation.decoded_matrix()))
}

/// Preserve matrix symmetry ordering without materialising every decoded element.
#[register_rule("ReprMatrixPacked", 9900, [LexLt, LexLeq])]
fn order_packed_matrices(expr: &Expr, _: &SymbolTable) -> ApplicationResult {
    let (lhs, rhs) = match expr {
        Expr::LexLt(_, lhs, rhs) | Expr::LexLeq(_, lhs, rhs) => (lhs, rhs),
        _ => return Err(RuleNotApplicable),
    };
    let [
        Expr::Atomic(_, Atom::Reference(lhs)),
        Expr::Atomic(_, Atom::Reference(rhs)),
    ] = [lhs.as_ref(), rhs.as_ref()]
    else {
        return Err(RuleNotApplicable);
    };
    let (Some(lhs), Some(rhs)) = (
        lhs.get_repr_as::<MatrixPacked>(),
        rhs.get_repr_as::<MatrixPacked>(),
    ) else {
        return Err(RuleNotApplicable);
    };
    if lhs.values != rhs.values || lhs.dimensions != rhs.dimensions {
        return Err(RuleNotApplicable);
    }
    let (lhs, rhs) = (Moo::new(lhs.packed_expr()), Moo::new(rhs.packed_expr()));
    Ok(RuleEffect::pure(match expr {
        Expr::LexLt(..) => Expr::Lt(Metadata::new(), lhs, rhs),
        Expr::LexLeq(..) => Expr::Leq(Metadata::new(), lhs, rhs),
        _ => unreachable!(),
    }))
}

#[cfg(test)]
mod tests {
    #[test]
    fn packed_matrix_rules_are_registered() {
        let rules = conjure_cp::rule_engine::get_rule_set_by_name("ReprMatrixPacked")
            .unwrap()
            .get_rules();
        assert!(
            rules
                .keys()
                .any(|rule| rule.name == "select_packed_matrices")
        );
    }
}
