use conjure_cp::rule_engine::register_rule;

use conjure_cp::{
    ast::{Atom, Expression as Expr, SymbolTable},
    rule_engine::{ApplicationError::RuleNotApplicable, ApplicationResult, Reduction},
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
fn substitute_value_lettings(expr: &Expr, _: &SymbolTable) -> ApplicationResult {
    let Expr::Atomic(_, Atom::Reference(decl)) = expr else {
        return Err(RuleNotApplicable);
    };

    let value = decl.as_value_letting().ok_or(RuleNotApplicable)?;

    Ok(Reduction::pure(value.clone()))
}

/// Substitutes domain lettings for their values in the symbol table.
#[register_rule(("Base", 5000))]
fn substitute_domain_lettings(expr: &Expr, symbols: &SymbolTable) -> ApplicationResult {
    let Expr::Root(_, _) = expr else {
        return Err(RuleNotApplicable);
    };

    let mut new_symbols = symbols.clone();
    let mut has_changed = false;

    for (_, mut decl) in symbols.clone().into_iter_local() {
        let Some(mut var) = decl.as_var().map(|x| x.clone()) else {
            continue;
        };

        let old_domain = var.domain;
        var.domain = old_domain.clone().resolve(symbols);
        if old_domain != var.domain {
            decl.as_var_mut().unwrap().domain = var.domain;
            has_changed = true;
            new_symbols.update_insert(decl);
        };
    }
    if has_changed {
        Ok(Reduction::with_symbols(expr.clone(), new_symbols))
    } else {
        Err(RuleNotApplicable)
    }
}
