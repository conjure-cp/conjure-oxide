use conjure_cp::{
    ast::{Atom, Expression as Expr, GroundDomain, Metadata, Name, SymbolTable, serde::HasId},
    bug,
    representation::Representation,
    rule_engine::{
        ApplicationError::RuleNotApplicable, ApplicationResult, RuleEffect, register_rule,
        register_rule_set,
    },
    settings::SolverFamily,
};
use itertools::Itertools;
use std::sync::Arc;
use std::sync::atomic::AtomicBool;
use std::sync::atomic::Ordering;
use uniplate::Biplate;

use conjure_cp::solver::adaptors::smt::{MatrixTheory, TheoryConfig};

register_rule_set!("Representations", ("Base"), |f: &SolverFamily| {
    if matches!(
        f,
        SolverFamily::Smt(TheoryConfig {
            matrices: MatrixTheory::Atomic,
            ..
        })
    ) {
        return true;
    }
    matches!(f, SolverFamily::Sat(_) | SolverFamily::Minion | SolverFamily::OrToolsCpSat)
});

// special case rule to select representations for matrices in one go.
//
// we know that they only have one possible representation, so this rule adds a representation for all matrices in the model.
#[register_rule("Representations", 8001, [Root])]
fn select_representation_matrix(expr: &Expr, symbols: &SymbolTable) -> ApplicationResult {
    let Expr::Root(_, _) = expr else {
        return Err(RuleNotApplicable);
    };

    let matrix_names: Vec<_> = matrix_variables(symbols).map(|(name, _)| name).collect();
    let expression_names = Biplate::<Name>::universe_bi(expr);
    let would_change = matrix_names.iter().any(|name| {
        symbols
            .representations_for(name)
            .is_some_and(|representations| representations.is_empty())
            || expression_names.contains(name)
    });
    if !would_change {
        return Err(RuleNotApplicable);
    }

    let expr = expr.clone();
    Ok(RuleEffect::deferred(move |symbols| {
        materialise_select_representation_matrix(&expr, symbols)
    }))
}

/// Returns local finite matrix variables supported by `matrix_to_atom`.
fn matrix_variables(
    symbols: &SymbolTable,
) -> impl Iterator<Item = (Name, conjure_cp::ast::serde::ObjId)> {
    symbols.clone().into_iter_local().filter_map(|(n, decl)| {
        let id = decl.id();
        let var = decl.as_find()?.clone();
        let resolved_domain = var.domain.resolve().ok()?;

        let GroundDomain::Matrix(valdom, indexdoms) = resolved_domain.as_ref() else {
            return None;
        };

        if !resolved_domain.is_finite() {
            return None;
        }

        if !matches!(valdom.as_ref(), GroundDomain::Bool | GroundDomain::Int(_)) {
            return None;
        }

        if indexdoms
            .iter()
            .any(|x| !matches!(x.as_ref(), GroundDomain::Bool | GroundDomain::Int(_)))
        {
            return None;
        }

        Some((n, id))
    })
}

/// Materialises matrix representations after the root rule is selected.
fn materialise_select_representation_matrix(expr: &Expr, symbols: &SymbolTable) -> RuleEffect {
    // cannot create representations on non-local variables, so use lookup_local.
    let matrix_vars = matrix_variables(symbols);

    let mut symbols = symbols.clone();
    let mut expr = expr.clone();
    let has_changed = Arc::new(AtomicBool::new(false));
    for (name, _id) in matrix_vars {
        // Even if we have no references to this matrix, still give it the matrix_to_atom
        // representation, as we still currently need to give it to minion even if its unused.
        //
        // If this var has no represnetation yet, the below call to get_or_add will modify the
        // symbol table by adding the representation and represented variable declarations to the
        // symbol table.
        let Some(existing_reprs) = symbols.representations_for(&name) else {
            continue;
        };
        if existing_reprs.is_empty() {
            has_changed.store(true, Ordering::Relaxed);
        }

        // (creates the represented variables as a side effect)
        let Some(_) = symbols.get_or_add_representation(&name, &["matrix_to_atom"]) else {
            continue;
        };

        let old_name = name.clone();
        let new_name =
            Name::WithRepresentation(Box::new(old_name.clone()), vec!["matrix_to_atom".into()]);
        // give all references to this matrix this representation
        // also do this inside subscopes, as long as they dont define their own variable that shadows this
        // one.

        let old_name_2 = old_name.clone();
        let new_name_2 = new_name.clone();
        let has_changed_ptr = Arc::clone(&has_changed);
        expr = expr.transform_bi(&move |n: Name| {
            if n == old_name_2 {
                has_changed_ptr.store(true, Ordering::SeqCst);
                new_name_2.clone()
            } else {
                n
            }
        });
    }

    debug_assert!(has_changed.load(Ordering::Relaxed));
    RuleEffect::with_symbols(expr, symbols)
}

#[register_rule("Representations", 8000, [Atomic / Reference])]
fn select_representation(expr: &Expr, symbols: &SymbolTable) -> ApplicationResult {
    // thing we are representing must be a reference
    let Expr::Atomic(_, Atom::Reference(decl)) = expr else {
        return Err(RuleNotApplicable);
    };

    let name: Name = decl.name().clone();

    // thing we are representing must be a variable
    {
        let guard = decl.ptr().as_find().ok_or(RuleNotApplicable)?;
        drop(guard);
    }

    if !needs_representation(&name, symbols) {
        return Err(RuleNotApplicable);
    }
    representation_name(&name, symbols).ok_or(RuleNotApplicable)?;

    let expr = expr.clone();
    Ok(RuleEffect::deferred(move |symbols| {
        materialise_select_representation(&expr, symbols)
    }))
}

/// Materialises the representation selected for one reference.
fn materialise_select_representation(expr: &Expr, symbols: &SymbolTable) -> RuleEffect {
    let Expr::Atomic(_, Atom::Reference(decl)) = expr else {
        unreachable!("representation effect was created for a reference")
    };
    let name = decl.name().clone();

    let mut symbols = symbols.clone();
    let representation = get_or_create_representation(&name, &mut symbols)
        .expect("applicable representation can be materialised");

    let representation_names = representation
        .into_iter()
        .map(|x| x.repr_name().into())
        .collect_vec();

    let new_name = Name::WithRepresentation(Box::new(name.clone()), representation_names);

    // HACK: this is suspicious, but hopefully will work until we clean up representations
    // properly...
    //
    // In general, we should not use names atall anymore, including for representations /
    // represented variables.
    //
    // * instead of storing the link from a variable that has a representation to the variable it
    // is representing in the name as WithRepresentation, we should use declaration pointers instead.
    //
    //
    // see: issue #932
    let mut decl_ptr = decl.clone().into_ptr().detach();
    decl_ptr.replace_name(new_name);

    RuleEffect::with_symbols(
        Expr::Atomic(
            Metadata::new(),
            Atom::Reference(conjure_cp::ast::Reference::new(decl_ptr)),
        ),
        symbols,
    )
}

/// Returns whether `name` needs representing.
///
fn needs_representation(name: &Name, symbols: &SymbolTable) -> bool {
    // if name already has a representation, false
    if matches!(name, Name::Represented(_) | Name::WithRepresentation(_, _)) {
        return false;
    }
    // might be more logic here in the future?
    symbols
        .resolve_domain(name)
        .is_some_and(|domain| domain_needs_representation(domain.as_ref()))
}

/// Returns whether `domain` needs representing.
fn domain_needs_representation(domain: &GroundDomain) -> bool {
    // very simple implementation for nows
    match domain {
        GroundDomain::Bool | GroundDomain::Int(_) => false,
        GroundDomain::Matrix(_, _) => false, // we special case these elsewhere
        GroundDomain::Set(_, _)
        | GroundDomain::MSet(_, _)
        | GroundDomain::Tuple(_)
        | GroundDomain::Record(_)
        | GroundDomain::Sequence(_, _)
        | GroundDomain::Function(_, _, _)
        | GroundDomain::Partition(_, _)
        | GroundDomain::Variant(_) => true,
        GroundDomain::Relation(_, _) => true,
        GroundDomain::Empty(_) => false,
    }
}

/// Returns representations for `name`, creating them if they don't exist.
///
///
/// Returns None if there is no valid representation for `name`.
///
fn get_or_create_representation(
    name: &Name,
    symbols: &mut SymbolTable,
) -> Option<Vec<Box<dyn Representation>>> {
    // TODO: pick representations recursively for nested abstract domains: e.g. sets in sets.

    let representation = representation_name(name, symbols)?;
    symbols.get_or_add_representation(name, &[representation])
}

/// Returns the supported representation for `name` without creating it.
fn representation_name(name: &Name, symbols: &SymbolTable) -> Option<&'static str> {
    let dom = symbols.resolve_domain(name)?;
    match dom.as_ref() {
        GroundDomain::Tuple(elem_domains) => {
            if elem_domains
                .iter()
                .any(|d| domain_needs_representation(d.as_ref()))
            {
                bug!("representing nested abstract domains is not implemented");
            }

            Some("tuple_to_atom")
        }
        GroundDomain::Record(entries) => {
            if entries
                .iter()
                .any(|entry| domain_needs_representation(&entry.value))
            {
                bug!("representing nested abstract domains is not implemented");
            }

            Some("record_to_atom")
        }
        _ => None,
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use conjure_cp::ast::{DeclarationPtr, Domain, Range};

    #[test]
    fn inserting_a_representation_does_not_lock_its_source_declaration() {
        let mut symbols = SymbolTable::new();
        let matrix_domain = Domain::matrix(
            Domain::bool(),
            vec![Domain::int(vec![Range::Bounded(1, 2)])],
        );
        let name = Name::user("matrix");
        symbols
            .insert(DeclarationPtr::new_find(name.clone(), matrix_domain))
            .unwrap();

        let representation = symbols.get_or_add_representation(&name, &["matrix_to_atom"]);

        assert!(representation.is_some());
    }

    #[test]
    fn represented_names_do_not_need_another_representation() {
        let symbols = SymbolTable::new();
        let original = Name::user("x");

        let represented_variable = Name::Represented(Box::new((
            original.clone(),
            "record_to_atom".into(),
            "1".into(),
        )));
        let represented_reference =
            Name::WithRepresentation(Box::new(original), vec!["record_to_atom".into()]);

        assert!(!needs_representation(&represented_variable, &symbols));
        assert!(!needs_representation(&represented_reference, &symbols));
    }
}
