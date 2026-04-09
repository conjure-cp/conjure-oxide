//! Debug-only structural assertions for AST/model integrity.
//!
//! The assertions in this module validate a few key invariants:
//! - `Root` exists exactly once, at the top-level model root, and nowhere else.
//! - all referenced names resolve to declarations present in reachable symbol tables.
//! - a combined model well-formedness check that applies all assertions.

use super::Model;

#[cfg(debug_assertions)]
use std::collections::{BTreeSet, HashSet};

#[cfg(debug_assertions)]
use super::{Expression, Name, Reference, SymbolTablePtr, serde::HasId};
#[cfg(debug_assertions)]
use uniplate::Biplate;

/// Debug-assert that a model is well-formed by applying all AST assertions in this module.
#[cfg(debug_assertions)]
pub fn debug_assert_model_well_formed(model: &Model, origin: &str) {
    debug_assert_root_at_top_level_only(model, origin);
    debug_assert_all_names_resolved(model, origin);
}

/// Debug-assert that a model is well-formed by applying all AST assertions in this module.
#[cfg(not(debug_assertions))]
pub fn debug_assert_model_well_formed(_model: &Model, _origin: &str) {}

/// Debug-assert that all names referenced by expressions/domains resolve to declared symbols.
#[cfg(debug_assertions)]
pub fn debug_assert_all_names_resolved(model: &Model, origin: &str) {
    let mut declared_names: BTreeSet<Name> = BTreeSet::new();
    let mut referenced_names: BTreeSet<Name> = BTreeSet::new();

    for table_ptr in collect_reachable_symbol_tables(model) {
        let table = table_ptr.read();

        for (name, decl) in table.iter_local() {
            declared_names.insert(name.clone());

            if let Some(expr) = decl.as_value_letting() {
                referenced_names.extend(Biplate::<Reference>::universe_bi(&*expr).into_iter().map(
                    |reference| {
                        let name = reference.name();
                        canonical_resolution_name(&name).clone()
                    },
                ));
            }

            if let Some(domain) = decl.domain() {
                referenced_names.extend(
                    Biplate::<Reference>::universe_bi(domain.as_ref())
                        .into_iter()
                        .map(|reference| {
                            let name = reference.name();
                            canonical_resolution_name(&name).clone()
                        }),
                );
            }
        }
    }

    referenced_names.extend(
        Biplate::<Reference>::universe_bi(model.root())
            .into_iter()
            .map(|reference| {
                let name = reference.name();
                canonical_resolution_name(&name).clone()
            }),
    );

    if let Some(dominance) = &model.dominance {
        referenced_names.extend(
            Biplate::<Reference>::universe_bi(dominance)
                .into_iter()
                .map(|reference| {
                    let name = reference.name();
                    canonical_resolution_name(&name).clone()
                }),
        );
    }

    for clause in model.clauses() {
        for literal in clause.iter() {
            referenced_names.extend(Biplate::<Reference>::universe_bi(literal).into_iter().map(
                |reference| {
                    let name = reference.name();
                    canonical_resolution_name(&name).clone()
                },
            ));
        }
    }

    let unresolved: Vec<Name> = referenced_names
        .difference(&declared_names)
        .cloned()
        .collect();

    debug_assert!(
        unresolved.is_empty(),
        "Model from '{origin}' contains unresolved names: {unresolved:?}"
    );
}

/// Debug-assert that all names referenced by expressions/domains resolve to declared symbols.
#[cfg(not(debug_assertions))]
pub fn debug_assert_all_names_resolved(_model: &Model, _origin: &str) {}

#[cfg(debug_assertions)]
fn canonical_resolution_name(name: &Name) -> &Name {
    match name {
        // Names wrapped in a selected representation still resolve through the source declaration.
        Name::WithRepresentation(inner, _) => canonical_resolution_name(inner),
        _ => name,
    }
}

/// Debug-assert that there is exactly one `Root` expression, and it is the model's top-level root.
#[cfg(debug_assertions)]
pub fn debug_assert_root_at_top_level_only(model: &Model, origin: &str) {
    debug_assert!(
        matches!(model.root(), Expression::Root(_, _)),
        "Model from '{origin}' does not have Root at top-level"
    );

    let root_count_in_main_tree = Biplate::<Expression>::universe_bi(model)
        .iter()
        .filter(|expr| matches!(expr, Expression::Root(_, _)))
        .count();

    let root_count_in_clauses = model
        .clauses()
        .iter()
        .flat_map(|clause| clause.iter())
        .map(|expr| {
            Biplate::<Expression>::universe_bi(expr)
                .iter()
                .filter(|inner| matches!(inner, Expression::Root(_, _)))
                .count()
        })
        .sum::<usize>();

    let total_root_count = root_count_in_main_tree + root_count_in_clauses;
    debug_assert_eq!(
        total_root_count, 1,
        "Model from '{origin}' should contain exactly one Root expression at top-level, found {total_root_count}"
    );
}

/// Debug-assert that there is exactly one `Root` expression, and it is the model's top-level root.
#[cfg(not(debug_assertions))]
pub fn debug_assert_root_at_top_level_only(_model: &Model, _origin: &str) {}

#[cfg(debug_assertions)]
fn collect_reachable_symbol_tables(model: &Model) -> Vec<SymbolTablePtr> {
    let mut pending_tables: Vec<SymbolTablePtr> = vec![model.symbols_ptr_unchecked().clone()];
    pending_tables.extend(Biplate::<SymbolTablePtr>::universe_bi(model.root()));

    if let Some(dominance) = &model.dominance {
        pending_tables.extend(Biplate::<SymbolTablePtr>::universe_bi(dominance));
    }

    for clause in model.clauses() {
        for literal in clause.iter() {
            pending_tables.extend(Biplate::<SymbolTablePtr>::universe_bi(literal));
        }
    }

    let mut seen_tables = HashSet::new();
    let mut out = Vec::new();

    while let Some(table_ptr) = pending_tables.pop() {
        if !seen_tables.insert(table_ptr.id()) {
            continue;
        }

        let parent = table_ptr.read().parent().clone();
        if let Some(parent) = parent {
            pending_tables.push(parent);
        }

        out.push(table_ptr);
    }

    out
}
