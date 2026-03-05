//! Rules for variables in domains.

use std::collections::HashMap;

use conjure_cp::{
    ast::{
        Atom, DecisionVariable, DeclarationKind, Domain, DomainPtr, Expression as Expr, HasDomain,
        IntVal, Literal as Lit, Metadata, Moo, Name, Range, Reference, SymbolTable,
    },
    rule_engine::{ApplicationError, ApplicationResult, Reduction, register_rule},
};
use uniplate::Biplate;

use ApplicationError::RuleNotApplicable;

type IntBoundsCache = HashMap<Name, (i32, i32)>;
type VisitingStack = Vec<Name>;

/// Rewrites variables in domains.
///
/// Solvers require variable declarations to have ground domains. For integer domains that contain variables in them, we widen to a finite ground superset-domain and add constraints that enforce membership in the original (possibly variable-dependent) domain.
#[register_rule(("Base", 8990))]
fn handle_variables_in_domains(expr: &Expr, symbols: &SymbolTable) -> ApplicationResult {
    let Expr::Root(_, _) = expr else {
        return Err(RuleNotApplicable);
    };

    if !symbols_have_decision_variable_references(symbols) {
        return Err(RuleNotApplicable);
    }

    let mut known_int_bounds: IntBoundsCache = HashMap::new();
    let mut domain_guards = Vec::new();
    let mut changed = false;

    // Collect declarations first to avoid iterator invalidation while mutating declarations.
    let declarations: Vec<_> = symbols
        .clone()
        .into_iter_local()
        .map(|(_, decl)| decl)
        .collect();

    for mut decl in declarations {
        let Some(domain) = decl.as_find().map(|var| var.domain_of()) else {
            continue;
        };

        if let Some(bounds) =
            int_domain_bounds_from_domain(&domain, symbols, &mut known_int_bounds, &mut Vec::new())
        {
            known_int_bounds.insert(decl.name().clone(), bounds);
        }

        if domain.resolve().is_some() {
            continue;
        }

        let Some(widened_domain) =
            resolve_or_widen_int_domain(&domain, symbols, &mut known_int_bounds, &mut Vec::new())
        else {
            return Err(RuleNotApplicable);
        };

        let Some(guards) = domain_consistency_constraints(&decl, &domain) else {
            return Err(RuleNotApplicable);
        };

        domain_guards.extend(guards);
        let _ = decl.replace_kind(DeclarationKind::Find(DecisionVariable::new(widened_domain)));
        changed = true;
    }

    if !changed {
        return Err(RuleNotApplicable);
    }

    Ok(Reduction::new(expr.clone(), domain_guards, symbols.clone()))
}

/// Returns true iff at least one local symbol contains a reference to a decision variable.
fn symbols_have_decision_variable_references(symbols: &SymbolTable) -> bool {
    let is_decision_reference = |reference: &Reference| {
        reference.ptr().as_find().is_some()
            || symbols
                .lookup(&reference.name().clone())
                .is_some_and(|decl| decl.as_find().is_some())
    };

    symbols.iter_local().any(|(_, declaration)| {
        declaration.domain().is_some_and(|domain| {
            Biplate::<Reference>::universe_bi(domain.as_ref())
                .iter()
                .any(&is_decision_reference)
                || Biplate::<IntVal>::universe_bi(domain.as_ref())
                    .iter()
                    .any(|int_val| match int_val {
                        IntVal::Const(_) => false,
                        IntVal::Reference(reference) => is_decision_reference(reference),
                        IntVal::Expr(expr) => Biplate::<Atom>::universe_bi(expr.as_ref())
                            .iter()
                            .any(|atom| {
                                matches!(atom, Atom::Reference(reference) if is_decision_reference(reference))
                            }),
                    })
        }) || declaration.as_find().is_some_and(|find| {
            let domain = find.domain_of();
            domain.resolve().is_none() && domain.as_ref().as_int().is_some()
        })
    })
}

/// Resolves an integer domain when possible; otherwise computes a finite widened domain
/// by replacing symbolic bounds with conservative numeric bounds.
fn resolve_or_widen_int_domain(
    domain: &DomainPtr,
    symbols: &SymbolTable,
    known_int_bounds: &mut IntBoundsCache,
    visiting: &mut VisitingStack,
) -> Option<DomainPtr> {
    if let Some(resolved) = domain.resolve() {
        return Some(resolved.into());
    }

    let ranges = domain.as_ref().as_int()?;
    let widened_ranges: Vec<Range<i32>> = ranges
        .iter()
        .map(|range| int_range_bounds(range, symbols, known_int_bounds, visiting))
        .map(|bounds| bounds.map(|(lo, hi)| Range::new(Some(lo), Some(hi))))
        .collect::<Option<Vec<_>>>()?;

    Some(Domain::int_ground(widened_ranges))
}

/// Returns overall numeric bounds for an unresolved integer domain.
fn int_domain_bounds_from_domain(
    domain: &DomainPtr,
    symbols: &SymbolTable,
    known_int_bounds: &mut IntBoundsCache,
    visiting: &mut VisitingStack,
) -> Option<(i32, i32)> {
    let ranges = domain.as_ref().as_int()?;
    let mut lower = i32::MAX;
    let mut upper = i32::MIN;

    for range in ranges {
        let (lo, hi) = int_range_bounds(&range, symbols, known_int_bounds, visiting)?;
        lower = lower.min(lo);
        upper = upper.max(hi);
    }

    Some((lower, upper))
}

/// Computes numeric bounds for a possibly symbolic integer range.
fn int_range_bounds(
    range: &Range<IntVal>,
    symbols: &SymbolTable,
    known_int_bounds: &mut IntBoundsCache,
    visiting: &mut VisitingStack,
) -> Option<(i32, i32)> {
    match range {
        Range::Single(v) => int_val_bounds(v, symbols, known_int_bounds, visiting),
        Range::Bounded(l, r) => {
            let (ll, lh) = int_val_bounds(l, symbols, known_int_bounds, visiting)?;
            let (rl, rh) = int_val_bounds(r, symbols, known_int_bounds, visiting)?;
            Some((ll.min(lh), rl.max(rh)))
        }
        Range::Unbounded | Range::UnboundedL(_) | Range::UnboundedR(_) => None,
    }
}

/// Computes numeric bounds for an unresolved integer value.
fn int_val_bounds(
    value: &IntVal,
    symbols: &SymbolTable,
    known_int_bounds: &mut IntBoundsCache,
    visiting: &mut VisitingStack,
) -> Option<(i32, i32)> {
    if let Some(v) = value.resolve() {
        return Some((v, v));
    }

    match value {
        IntVal::Const(v) => Some((*v, *v)),
        IntVal::Reference(reference) => {
            let name = reference.name().clone();
            int_bounds_for_name(&name, symbols, known_int_bounds, visiting)
        }
        IntVal::Expr(expr) => {
            let domain = expression_int_bounds(expr, symbols, known_int_bounds, visiting)?;
            int_domain_bounds_from_domain(&domain, symbols, known_int_bounds, visiting)
        }
    }
}

/// Resolves cached or derived bounds for a declaration by name.
fn int_bounds_for_name(
    name: &Name,
    symbols: &SymbolTable,
    known_int_bounds: &mut IntBoundsCache,
    visiting: &mut VisitingStack,
) -> Option<(i32, i32)> {
    if let Some(bounds) = known_int_bounds.get(name).copied() {
        return Some(bounds);
    }

    if visiting.contains(name) {
        return None;
    }

    visiting.push(name.clone());
    let maybe_bounds = symbols
        .lookup(name)
        .and_then(|decl| decl.domain())
        .and_then(|domain| {
            int_domain_bounds_from_domain(&domain, symbols, known_int_bounds, visiting)
        });
    visiting.pop();
    let bounds = maybe_bounds?;
    known_int_bounds.insert(name.clone(), bounds);
    Some(bounds)
}

/// Computes a conservative ground integer domain for an expression.
fn expression_int_bounds(
    expr: &Moo<Expr>,
    symbols: &SymbolTable,
    known_int_bounds: &mut IntBoundsCache,
    visiting: &mut VisitingStack,
) -> Option<DomainPtr> {
    if let Some(Lit::Int(v)) = conjure_cp::ast::eval_constant(expr) {
        return Some(Domain::int_ground(vec![Range::Single(v)]));
    }

    let domain = expr.as_ref().domain_of()?;
    resolve_or_widen_int_domain(&domain, symbols, known_int_bounds, visiting)
}

/// Builds guards ensuring widened integer find domains still satisfy original symbolic bounds.
fn domain_consistency_constraints(
    declaration: &conjure_cp::ast::DeclarationPtr,
    original_domain: &DomainPtr,
) -> Option<Vec<Expr>> {
    if original_domain.resolve().is_some() {
        return Some(Vec::new());
    }

    let ranges = original_domain.as_ref().as_int()?;
    if ranges.is_empty() {
        return None;
    }

    let var_expr = Expr::Atomic(
        Metadata::new(),
        Atom::Reference(Reference::new(declaration.clone())),
    );
    let mut allowed_intervals = Vec::new();

    for range in ranges {
        let interval = match range {
            Range::Single(v) => Expr::Eq(
                Metadata::new(),
                Moo::new(var_expr.clone()),
                Moo::new(Expr::from(v)),
            ),
            Range::Bounded(l, r) => {
                let geq = Expr::Geq(
                    Metadata::new(),
                    Moo::new(var_expr.clone()),
                    Moo::new(Expr::from(l)),
                );
                let leq = Expr::Leq(
                    Metadata::new(),
                    Moo::new(var_expr.clone()),
                    Moo::new(Expr::from(r)),
                );
                Expr::And(
                    Metadata::new(),
                    Moo::new(conjure_cp::into_matrix_expr!(vec![geq, leq])),
                )
            }
            Range::Unbounded | Range::UnboundedL(_) | Range::UnboundedR(_) => return None,
        };
        allowed_intervals.push(interval);
    }

    let guard = if allowed_intervals.len() == 1 {
        allowed_intervals.remove(0)
    } else {
        Expr::Or(
            Metadata::new(),
            Moo::new(conjure_cp::into_matrix_expr!(allowed_intervals)),
        )
    };

    Some(vec![guard])
}
