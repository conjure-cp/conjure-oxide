use conjure_core::ast::Expression as Expr;
use conjure_core::ast::SymbolTable;
use conjure_core::rule_engine::{
    register_rule, ApplicationError::RuleNotApplicable, ApplicationResult, Reduction,
};

use conjure_core::ast::Atom;
use conjure_core::ast::Domain;
use conjure_core::ast::Name;
use conjure_core::metadata::Metadata;

use itertools::Itertools;

#[register_rule(("CNF", 8000))]
fn integer_representation(expr: &Expr, symbols: &SymbolTable) -> ApplicationResult {
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

    let Domain::IntDomain(_) = &symbols.resolve_domain(name).unwrap() else {
        return Err(RuleNotApplicable);
    };

    let mut symbols = symbols.clone();
    let representation = symbols
        .get_or_add_representation(name, &["int_to_atom"])
        .ok_or(RuleNotApplicable)?;

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
