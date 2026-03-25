use conjure_cp::{
    ast::{Atom, Expression as Expr, GroundDomain, Metadata, Name, SymbolTable, serde::HasId},
    bug,
    representation::Representation,
    rule_engine::{
        ApplicationError::RuleNotApplicable, ApplicationResult, Reduction, register_rule,
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
    matches!(f, SolverFamily::Sat(_) | SolverFamily::Minion)
});

#[register_rule(("Representations", 8000))]
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
    let mut decl_ptr = decl.clone().into_ptr().detach();
    decl_ptr.replace_name(new_name);

    Ok(Reduction::with_symbols(
        Expr::Atomic(
            Metadata::new(),
            Atom::Reference(conjure_cp::ast::Reference::new(decl_ptr)),
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
    // if name already has a representation, false
    if let Name::Represented(_) = name {
        return false;
    }
    // might be more logic here in the future?
    domain_needs_representation(&symbols.resolve_domain(name).unwrap())
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
        | GroundDomain::Function(_, _, _) => true,
        GroundDomain::Empty(_) => false,
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

    let dom = symbols.resolve_domain(name).unwrap();
    match dom.as_ref() {
        GroundDomain::Set(_, _) => None, // has no representations yet!
        GroundDomain::Tuple(elem_domains) => {
            if elem_domains
                .iter()
                .any(|d| domain_needs_representation(d.as_ref()))
            {
                bug!("representing nested abstract domains is not implemented");
            }

            symbols.get_or_add_representation(name, &["tuple_to_atom"])
        }
        GroundDomain::Record(entries) => {
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
