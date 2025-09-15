use conjure_cp::{
    ast::Metadata,
    ast::{Atom, Domain, Expression as Expr, Name, SubModel, SymbolTable, serde::HasId},
    bug,
    representation::Representation,
    rule_engine::{
        ApplicationError::RuleNotApplicable, ApplicationResult, Reduction, register_rule,
    },
};
use itertools::Itertools;
use std::sync::Arc;
use std::sync::atomic::AtomicBool;
use std::sync::atomic::Ordering;
use uniplate::Biplate;
// special case rule to select representations for matrices in one go.
//
// we know that they only have one possible representation, so this rule adds a representation for all matrices in the model.
#[register_rule(("Base", 8001))]
fn select_representation_matrix(expr: &Expr, symbols: &SymbolTable) -> ApplicationResult {
    let Expr::Root(_, _) = expr else {
        return Err(RuleNotApplicable);
    };

    // cannot create representations on non-local variables, so use lookup_local.
    let matrix_vars = symbols
        .clone()
        .into_iter_local()
        .filter_map(|(n, decl)| {
            let id = decl.id();
            decl.as_var().map(|x| (n, id, x.clone()))
        })
        .filter(|(_, _, var)| {
            let Domain::Matrix(valdom, indexdoms) = &var.domain else {
                return false;
            };

            // TODO: loosen these requirements once we are able to
            if !matches!(valdom.as_ref(), Domain::Bool | Domain::Int(_)) {
                return false;
            }

            if indexdoms
                .iter()
                .any(|x| !matches!(x, Domain::Bool | Domain::Int(_)))
            {
                return false;
            }

            true
        });

    let mut symbols = symbols.clone();
    let mut expr = expr.clone();
    let has_changed = Arc::new(AtomicBool::new(false));
    for (name, id, _) in matrix_vars {
        // Even if we have no references to this matrix, still give it the matrix_to_atom
        // representation, as we still currently need to give it to minion even if its unused.
        //
        // If this var has no represnetation yet, the below call to get_or_add will modify the
        // symbol table by adding the representation and represented variable declarations to the
        // symbol table.
        if symbols.representations_for(&name).unwrap().is_empty() {
            has_changed.store(true, Ordering::Relaxed);
        }

        // (creates the represented variables as a side effect)
        let _ = symbols
            .get_or_add_representation(&name, &["matrix_to_atom"])
            .unwrap();

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

        let has_changed_ptr = Arc::clone(&has_changed);
        let old_name = old_name.clone();
        let new_name = new_name.clone();
        expr = expr.transform_bi(&move |mut x: SubModel| {
            let old_name = old_name.clone();
            let new_name = new_name.clone();
            let has_changed_ptr = Arc::clone(&has_changed_ptr);

            // only do things if this inscope and not shadowed..
            if x.symbols().lookup(&old_name).is_none_or(|x| x.id() == id) {
                let root = x.root_mut_unchecked();
                *root = root.transform_bi(&move |n: Name| {
                    if n == old_name {
                        has_changed_ptr.store(true, Ordering::SeqCst);
                        new_name.clone()
                    } else {
                        n
                    }
                });
            }
            x
        });
    }

    if has_changed.load(Ordering::Relaxed) {
        Ok(Reduction::with_symbols(expr, symbols))
    } else {
        Err(RuleNotApplicable)
    }
}

#[register_rule(("Base", 8000))]
fn select_representation(expr: &Expr, symbols: &SymbolTable) -> ApplicationResult {
    // thing we are representing must be a reference
    let Expr::Atomic(_, Atom::Reference(decl)) = expr else {
        return Err(RuleNotApplicable);
    };

    let name: Name = decl.name().clone();

    // thing we are representing must be a variable
    {
        decl.as_var().ok_or(RuleNotApplicable)?;
    }

    if !needs_representation(&name, symbols) {
        return Err(RuleNotApplicable);
    }

    let mut symbols = symbols.clone();
    let representation =
        get_or_create_representation(&name, &mut symbols).ok_or(RuleNotApplicable)?;

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
    let mut decl = decl.clone().detach();
    decl.replace_name(new_name);

    Ok(Reduction::with_symbols(
        Expr::Atomic(Metadata::new(), Atom::Reference(decl)),
        symbols,
    ))
}

/// Returns whether `name` needs representing.
///
/// # Panics
///
///   + If `name` is not in `symbols`.
fn needs_representation(name: &Name, symbols: &SymbolTable) -> bool {
    // if name already has a representation, false
    if let Name::Represented(_) = name {
        return false;
    }
    // might be more logic here in the future?
    domain_needs_representation(&symbols.resolve_domain(name).unwrap())
}

/// Returns whether `domain` needs representing.
fn domain_needs_representation(domain: &Domain) -> bool {
    // very simple implementation for nows
    match domain {
        Domain::Bool | Domain::Int(_) => false,
        Domain::Matrix(_, _) => false, // we special case these elsewhere
        Domain::Set(_, _) | Domain::Tuple(_) | Domain::Record(_) => true,
        Domain::Reference(_) => unreachable!("domain should be resolved"),
        Domain::Empty(_) => false, // _ => false,
    }
}

/// Returns representations for `name`, creating them if they don't exist.
///
///
/// Returns None if there is no valid representation for `name`.
///
/// # Panics
///
///   + If `name` is not in `symbols`.
fn get_or_create_representation(
    name: &Name,
    symbols: &mut SymbolTable,
) -> Option<Vec<Box<dyn Representation>>> {
    // TODO: pick representations recursively for nested abstract domains: e.g. sets in sets.

    match symbols.resolve_domain(name).unwrap() {
        Domain::Set(_, _) => None, // has no representations yet!
        Domain::Tuple(elem_domains) => {
            if elem_domains.iter().any(domain_needs_representation) {
                bug!("representing nested abstract domains is not implemented");
            }

            symbols.get_or_add_representation(name, &["tuple_to_atom"])
        }
        Domain::Record(entries) => {
            if entries
                .iter()
                .any(|entry| domain_needs_representation(&entry.domain))
            {
                bug!("representing nested abstract domains is not implemented");
            }

            symbols.get_or_add_representation(name, &["record_to_atom"])
        }
        _ => unreachable!("non abstract domains should never need representations"),
    }
}
