use conjure_core::{
    ast::{Atom, Domain, Expression as Expr, Name, SymbolTable},
    bug,
    metadata::Metadata,
    representation::Representation,
    rule_engine::{
        register_rule, ApplicationError::RuleNotApplicable, ApplicationResult, Reduction,
    },
};
<<<<<<< HEAD:crates/conjure_rules/src/select_representation.rs

use itertools::Itertools;
=======
use itertools::Itertools;
use std::cell::RefCell;
use std::rc::Rc;

use crate::ast::Atom;
use crate::ast::Declaration;
use crate::ast::Domain;
use crate::ast::Name;
use crate::bug;
use crate::metadata::Metadata;
use crate::representation::Representation;
>>>>>>> 88926d4c4 (refactor: remove option from rc<refcell<>> definition):crates/conjure_core/src/rules/select_representation.rs

#[register_rule(("Base", 8000))]
fn select_representation(expr: &Expr, symbols: &SymbolTable) -> ApplicationResult {
    // thing we are representing must be a reference
    let Expr::Atomic(_, Atom::Reference(name, _)) = expr else {
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
        Expr::Atomic(
            Metadata::new(),
            Atom::Reference(new_name, Rc::new(RefCell::new(Declaration::default()))),
        ),
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
    // very simple implementation for nows
    match domain {
        Domain::BoolDomain | Domain::IntDomain(_) => false,
        Domain::DomainSet(_, _) | Domain::DomainMatrix(_, _) => true,
        Domain::DomainReference(_) => unreachable!("domain should be resolved"),
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
        Domain::DomainSet(_, _) => None,
        Domain::DomainMatrix(elem_domain, _) => {
            // easy, only one possible representation

            if domain_needs_representation(elem_domain.as_ref()) {
                bug!("representing nested abstract domains is not implemented");
            }

            symbols.get_or_add_representation(name, &["matrix_to_atom"])
        }
        _ => unreachable!("non abstract domains should never need representations"),
    }
}
