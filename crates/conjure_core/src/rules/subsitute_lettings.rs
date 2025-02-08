use conjure_macros::register_rule;

use crate::{
    ast::{Atom, Domain, Expression as Expr},
    bug,
    rule_engine::{ApplicationError::RuleNotApplicable, ApplicationResult, Reduction},
    Model,
};

/// Substitutes value lettings for their values.
///
/// # Priority
///
/// This rule must have a higher priority than solver-flattening rules (which should be priority 4000).
///
/// Otherwise, the letting may be put into a flat constraint, as it is a reference. At this point
/// it ceases to be an expression, so we cannot match over it.
#[register_rule(("Base", 5000))]
fn substitute_value_lettings(expr: &Expr, m: &Model) -> ApplicationResult {
    let Expr::Atomic(_, Atom::Reference(name)) = expr else {
        return Err(RuleNotApplicable);
    };

    let value = m
        .symbols()
        .get_value_letting(name)
        .ok_or(RuleNotApplicable)?;

    Ok(Reduction::pure(value.clone()))
}

/// Substitutes domain lettings for their values in the symbol table.
#[register_rule(("Base", 5000))]
fn substitute_domain_lettings(expr: &Expr, m: &Model) -> ApplicationResult {
    let mut model = m.clone();
    let mut has_changed = false;
    for name in m.symbols().names() {
        if let Some(d) = model.symbols_mut().domain_of_mut(name) {
            // For some reason, Rust didn't want to do this match in one go.
            //
            // if let Some(d @ Domain::Reference(domain_name)) gave borrow checker errors.
            if let Domain::DomainReference(domain_name) = d {
                *d = m
                    .symbols()
                    .get_domain_letting(&domain_name.clone())
                    .unwrap_or_else(|| {
                        bug!(
                            "rule substitute_domain_lettings: domain reference {} does not exist",
                            domain_name
                        )
                    })
                    .clone();

                has_changed = true;
            }
        }
    }

    if has_changed {
        Ok(Reduction::with_symbols(
            expr.clone(),
            model.symbols().clone(),
        ))
    } else {
        Err(RuleNotApplicable)
    }
}
