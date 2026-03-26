use crate::guard;
use crate::utils::as_comparison_op;
use conjure_cp::ast::{Domain, DomainPtr, HasDomain, UnresolvedDomain};
use conjure_cp::{
    ast::{Atom, Expression as Expr, GroundDomain, SymbolTable},
    representation::get_repr_rules,
    rule_engine::{
        ApplicationError::RuleNotApplicable, ApplicationResult, Reduction, register_rule,
        register_rule_set,
    },
};
use itertools::any;
use std::collections::VecDeque;
use uniplate::Biplate;

// Representations of Essence abstract types down to Essence'
// Applies for all solvers
register_rule_set!("ReprGeneral", ("Base"));

/// Select a representation for abstract domains
#[register_rule(("Representations", 8000))]
fn select_representation(expr: &Expr, _: &SymbolTable) -> ApplicationResult {
    guard!(
        let Expr::Atomic(_, Atom::Reference(re)) = expr &&
        domain_needs_representation(&re.domain_of())    &&
        re.repr.is_none()
        else {
            return Err(RuleNotApplicable)
        }
    );

    let mut re = re.clone();
    for rule in get_repr_rules() {
        // Once we find an applicable representation, exit
        let Ok((_, new_symbols, new_constraints)) = re.select_or_init_repr_via(rule) else {
            continue;
        };
        return Ok(Reduction::new(re.into(), new_constraints, new_symbols));
    }

    // None of the representations worked
    Err(RuleNotApplicable)
}

/// In a comparison operation, it is probably a good idea for the LHS and RHS to
/// have the same representation, if applicable; E.g:
/// ```plain
/// x#MyRepr > y
/// ~>
/// x#MyRepr > y#MyRepr
/// ```
#[register_rule(("Representations", 9000))]
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
fn domain_needs_representation(domain: &DomainPtr) -> bool {
    match domain.as_ref() {
        Domain::Ground(gd) => match gd.as_ref() {
            // These domains are concrete for all solvers bar SAT
            GroundDomain::Bool | GroundDomain::Int(..) | GroundDomain::Empty(..) => false,
            // Represent matrices if they have abstract types inside them;
            // Matrices of concrete types are handled separately by the
            // `ReprMatrixToAtom`rule set
            GroundDomain::Matrix(inner_dom, idx_doms) => {
                domain_needs_representation(&inner_dom.into())
                    || any(idx_doms, |d| domain_needs_representation(&d.into()))
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
                domain_needs_representation(inner_dom) || any(idx_doms, domain_needs_representation)
            }
            // Recurse into domain letting
            UnresolvedDomain::Reference(re) => domain_needs_representation(&re.domain_of()),
            // All other domains are abstract
            _ => true,
        },
    }
}
