use conjure_core::{
    ast::{serde::HasId, Atom, Domain, Expression as Expr, Name, SubModel, SymbolTable},
    bug,
    metadata::Metadata,
    representation::Representation,
    rule_engine::{
        register_rule, ApplicationError::RuleNotApplicable, ApplicationResult, Reduction,
    },
};
use itertools::Itertools;
use std::sync::atomic::AtomicBool;
use std::sync::atomic::Ordering;
use std::sync::Arc;
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
            decl.as_var().cloned().map(|x| (n, id, x))
        })
        .filter(|(_, _, var)| {
            let Domain::DomainMatrix(valdom, indexdoms) = &var.domain else {
                return false;
            };

            // TODO: loosen these requirements once we are able to
            if !matches!(valdom.as_ref(), Domain::BoolDomain | Domain::IntDomain(_)) {
                return false;
            }

            if indexdoms
                .iter()
                .any(|x| !matches!(x, Domain::BoolDomain | Domain::IntDomain(_)))
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
        let new_name = Name::WithRepresentation(
            Box::new(old_name.clone()),
            vec!["matrix_to_atom".to_owned()],
        );
        // give all references to this matrix this representation
        // also do this inside subscopes, as long as they dont define their own variable that shadows this
        // one.

        let old_name_2 = old_name.clone();
        let new_name_2 = new_name.clone();
        let has_changed_ptr = Arc::clone(&has_changed);
        expr = expr.transform_bi(Arc::new(move |n: Name| {
            if n == old_name_2 {
                has_changed_ptr.store(true, Ordering::SeqCst);
                new_name_2.clone()
            } else {
                n
            }
        }));

        let has_changed_ptr = Arc::clone(&has_changed);
        let old_name = old_name.clone();
        let new_name = new_name.clone();
        expr = expr.transform_bi(Arc::new(move |mut x: SubModel| {
            let old_name = old_name.clone();
            let new_name = new_name.clone();
            let has_changed_ptr = Arc::clone(&has_changed_ptr);

            // only do things if this inscope and not shadowed..
            if x.symbols()
                .lookup(&old_name)
                .is_none_or(|x| x.as_ref().id() == id)
            {
                let root = x.root_mut_unchecked();
                *root = root.transform_bi(Arc::new(move |n: Name| {
                    if n == old_name {
                        has_changed_ptr.store(true, Ordering::SeqCst);
                        new_name.clone()
                    } else {
                        n
                    }
                }));
            }
            x
        }));
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
    let Expr::Atomic(_, Atom::Reference(name)) = expr else {
        return Err(RuleNotApplicable);
    };

    // thing we are representing must be a variable
    symbols
        .lookup(name)
        .ok_or(RuleNotApplicable)?
        .as_var()
        .ok_or(RuleNotApplicable)?;

    if !needs_representation(name, symbols) {
        return Err(RuleNotApplicable);
    }

    let mut symbols = symbols.clone();
    let representation =
        get_or_create_representation(name, &mut symbols).ok_or(RuleNotApplicable)?;

    let representation_names = representation
        .into_iter()
        .map(|x| x.repr_name().to_string())
        .collect_vec();

    let new_name = Name::WithRepresentation(Box::new(name.clone()), representation_names);

    Ok(Reduction::with_symbols(
        Expr::Atomic(Metadata::new(), Atom::Reference(new_name)),
        symbols,
    ))
}

/// Returns whether `name` needs representing.
///
/// # Panics
///
///   + If `name` is not in `symbols`.
fn needs_representation(name: &Name, symbols: &SymbolTable) -> bool {
    // might be more logic here in the future?
    domain_needs_representation(&symbols.resolve_domain(name).unwrap())
}

/// Returns whether `domain` needs representing.
fn domain_needs_representation(domain: &Domain) -> bool {
    // very simple implementation for now
    match domain {
        Domain::BoolDomain | Domain::IntDomain(_) => false,
        Domain::DomainMatrix(_, _) => false, // we special case these elsewhere
        Domain::DomainSet(_, _) | Domain::DomainTuple(_) | Domain::DomainRecord(_) => true,
        Domain::DomainReference(_) => unreachable!("domain should be resolved"),
        // _ => false,
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
        Domain::DomainSet(_, _) => None, // has no representations yet!
        Domain::DomainTuple(elem_domains) => {
            if elem_domains.iter().any(domain_needs_representation) {
                bug!("representing nested abstract domains is not implemented");
            }

            symbols.get_or_add_representation(name, &["tuple_to_atom"])
        }
        Domain::DomainRecord(entries) => {
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
