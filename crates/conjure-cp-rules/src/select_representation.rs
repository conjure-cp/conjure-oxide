use crate::guard;
use crate::representation::tuple_packed::TuplePacked;
use crate::utils::as_comparison_op;
use conjure_cp::ast::{Domain, DomainPtr, HasDomain, UnresolvedDomain};
use conjure_cp::{
    ast::{Atom, Expression as Expr, GroundDomain, SymbolTable},
    representation::{ReprRule, get_repr_rules},
    rule_engine::{
        ApplicationError::RuleNotApplicable, ApplicationResult, Reduction, register_rule,
        register_rule_set,
    },
    settings::{SatEncoding, SolverFamily},
};
use itertools::any;
use std::collections::VecDeque;
use uniplate::Biplate;

/// Representations that should not be auto-selected by `select_representation`.
/// These are managed by their own dedicated rule sets.
const SKIP_AUTO_SELECT: &[&str] = &[TuplePacked::NAME];

// Representations of Essence abstract types down to Essence'
// Applies for all solvers
register_rule_set!("ReprGeneral", ("Base"), |_| true);

/// Select a representation for abstract domains
#[register_rule(("ReprGeneral", 8100))]
fn select_representation(expr: &Expr, _: &SymbolTable) -> ApplicationResult {
    guard!(
        let Expr::Atomic(_, Atom::Reference(re)) = expr &&
        domain_needs_representation(&re.domain_of(), SolverFamily::Sat(SatEncoding::Direct))    &&
        re.repr.is_none()
        else {
            return Err(RuleNotApplicable)
        }
    );

    let mut re = re.clone();
    for rule in get_repr_rules() {
        // Skip representations that are managed by their own rule sets
        if SKIP_AUTO_SELECT.contains(&rule.name()) {
            continue;
        }
        // Once we find an applicable representation, exit
        let Ok((_, new_symbols, new_constraints)) = re.select_or_init_repr_via(rule) else {
            continue;
        };
        return Ok(Reduction::new(re.into(), new_constraints, new_symbols));
    }

    // None of the representations worked
    Err(RuleNotApplicable)
}

/// Select a representation for unconstrained finds with abstract domains
#[register_rule(("ReprGeneral", 8000))]
fn select_representation_unconstrained(expr: &Expr, symtab: &SymbolTable) -> ApplicationResult {
    let Expr::Root(..) = expr else {
        return Err(RuleNotApplicable);
    };

    let mut symbols = symtab.clone();
    let mut constraints = Vec::<Expr>::new();
    for (_, decl) in symtab.iter_local() {
        // We want unrepresented decision vars!
        guard!(
            decl.as_find().is_some()          &&
            decl.reprs().is_empty()           &&
            let Some(dom) = decl.domain()     &&
            domain_needs_representation(&dom, SolverFamily::Sat(SatEncoding::Direct))
            else {
                continue;
            }
        );

        for rule in get_repr_rules() {
            // Skip representations that are managed by their own rule sets
            if SKIP_AUTO_SELECT.contains(&rule.name()) {
                continue;
            }
            let mut decl = decl.clone();

            // Once we find an applicable representation, exit
            let Ok((new_symbols, new_constraints)) = rule.init_for(&mut decl) else {
                continue;
            };
            symbols.update_insert(decl);
            symbols.extend(new_symbols);
            constraints.extend(new_constraints);
        }
    }

    if symbols.eq(symtab) && constraints.is_empty() {
        Err(RuleNotApplicable)
    } else {
        Ok(Reduction::new(expr.clone(), constraints, symbols))
    }
}

/// In a comparison operation, it is probably a good idea for the LHS and RHS to
/// have the same representation, if applicable; E.g:
/// ```plain
/// x#MyRepr > y
/// ~>
/// x#MyRepr > y#MyRepr
/// ```
#[register_rule(("ReprGeneral", 9000))]
fn uniform_repr_in_comparison_op(expr: &Expr, _: &SymbolTable) -> ApplicationResult {
    guard! {
        let Some((lhs, rhs)) = as_comparison_op(expr)               &&
        let Expr::Atomic(_, Atom::Reference(lhs_re)) = lhs.as_ref() &&
        let Expr::Atomic(_, Atom::Reference(rhs_re)) = rhs.as_ref()
        else {
            return Err(RuleNotApplicable)
        }
    }

    match (lhs_re.get_repr(), rhs_re.get_repr()) {
        (Some((lhs_rule, _)), None) => {
            let mut new_rhs = rhs_re.clone();
            let (_, symbols, constraints) = new_rhs
                .select_or_init_repr_via(lhs_rule)
                .map_err(|_| RuleNotApplicable)?;
            let new_expr = expr.with_children_bi(VecDeque::from([lhs.clone(), new_rhs.into()]));
            Ok(Reduction::new(new_expr, constraints, symbols))
        }
        (None, Some((rhs_rule, _))) => {
            let mut new_lhs = lhs_re.clone();
            let (_, symbols, constraints) = new_lhs
                .select_or_init_repr_via(rhs_rule)
                .map_err(|_| RuleNotApplicable)?;
            let new_expr = expr.with_children_bi(VecDeque::from([new_lhs.into(), rhs.clone()]));
            Ok(Reduction::new(new_expr, constraints, symbols))
        }
        _ => Err(RuleNotApplicable),
    }
}

/// True if the domain is abstract w.r.t Essence'
#[allow(clippy::match_like_matches_macro)]
fn domain_needs_representation(domain: &DomainPtr, solver: SolverFamily) -> bool {
    eprintln!("\n=====DOMAIN {}=====\n", domain);
    match domain.as_ref() {
        Domain::Ground(gd) => match gd.as_ref() {
            // These domains are concrete for all solvers
            GroundDomain::Bool | GroundDomain::Empty(..) => false,
            // Int is concrete for all solvers other than sat
            // This works because we are effectively saying: "is solver is sat, true (needs repr); for any other solver, false (doesn't need repr)"
            // NOTE: This is what needs changing IF there is ever another solver with different primitives
            GroundDomain::Int(_) => match solver {
                // true for sat solvers, because it needs a representation
                SolverFamily::Sat(_) => true,
                // false for everything else, because they don't need reprs
                _ => false,
            },
            // Represent matrices if they have abstract types inside them;
            // Matrices of concrete types are handled separately by the
            // `ReprMatrixToAtom`rule set
            GroundDomain::Matrix(inner_dom, idx_doms) => {
                domain_needs_representation(&inner_dom.into(), solver)
                    || any(idx_doms, |d| domain_needs_representation(&d.into(), solver))
            }
            // All other domains are abstract
            _ => true,
        },
        Domain::Unresolved(ud) => match ud.as_ref() {
            // Int domains are concrete for all solvers bar SAT
            UnresolvedDomain::Int(..) => false,
            // Represent matrices if they have abstract types inside them;
            // Matrices of concrete types are handled separately by the
            // `ReprMatrixToAtom`rule set
            UnresolvedDomain::Matrix(inner_dom, idx_doms) => {
                domain_needs_representation(inner_dom, solver)
                    || any(idx_doms, |d| domain_needs_representation(d, solver))
            }
            // Recurse into domain letting
            UnresolvedDomain::Reference(re) => domain_needs_representation(&re.domain_of(), solver),
            // All other domains are abstract
            _ => true,
        },
    }
}
