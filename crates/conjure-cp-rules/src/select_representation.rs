use crate::guard;
use crate::utils::as_comparison_op;
use conjure_cp::ast::{Domain, DomainPtr, HasDomain, UnresolvedDomain};
use conjure_cp::settings::{
    Channelling, Heuristic, channelling, heuristic, next_heuristic_all_index,
    next_heuristic_random_index,
};
use conjure_cp::{
    ast::{Atom, DeclarationPtr, Expression as Expr, GroundDomain, SymbolTable},
    representation::{ReprRulePtr, get_repr_rules},
    rule_engine::{
        ApplicationError::RuleNotApplicable, ApplicationResult, RuleEffect as Reduction,
        register_rule, register_rule_set,
    },
};
use itertools::any;
use std::collections::VecDeque;
use uniplate::Uniplate;

// Representations of Essence abstract types down to Essence'
// Applies for all solvers
register_rule_set!("ReprGeneral", ("Base"), |_| true);

/// Select a representation for abstract domains
#[register_rule("ReprGeneral", 10000, [Atomic / Reference])]
fn select_representation(expr: &Expr, _: &SymbolTable) -> ApplicationResult {
    guard!(
        let Expr::Atomic(_, Atom::Reference(re)) = expr &&
        domain_needs_representation(&re.domain_of()) &&
        re.repr.is_none()
        else {
            return Err(RuleNotApplicable)
        }
    );

    let mut re = re.clone();
    let Some(rule) = choose_representation_rule(re.ptr()) else {
        return Err(RuleNotApplicable);
    };
    let (_, new_symbols, new_constraints) = re
        .select_or_init_repr_via(rule)
        .map_err(|_| RuleNotApplicable)?;
    Ok(Reduction::new(re.into(), new_constraints, new_symbols))
}

/// Select a representation for unconstrained finds with abstract domains
#[register_rule("ReprGeneral", 9900, [Root])]
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
            domain_needs_representation(&dom)
            else {
                continue;
            }
        );

        let Some(rule) = choose_representation_rule(&decl) else {
            continue;
        };
        let mut decl = decl.clone();
        let Ok((new_symbols, new_constraints)) = rule.init_for(&mut decl) else {
            continue;
        };
        symbols.update_insert(decl);
        symbols.extend(new_symbols);
        constraints.extend(new_constraints);
    }

    if symbols.eq(symtab) && constraints.is_empty() {
        Err(RuleNotApplicable)
    } else {
        Ok(Reduction::new(expr.clone(), constraints, symbols))
    }
}

/// Chooses one applicable representation without mutating the declaration.
fn choose_representation_rule(decl: &DeclarationPtr) -> Option<ReprRulePtr> {
    if channelling() == Channelling::No
        && let Some(existing) = decl.reprs().iter().next().map(|(_, state)| state.rule())
    {
        return Some(existing);
    }

    let mut candidates: Vec<_> = get_repr_rules()
        .filter_map(|rule| rule.probe_for(decl).ok().map(|score| (rule, score)))
        .collect();
    candidates.sort_by_key(|(rule, _)| rule.name());

    if candidates.len() == 1 {
        return candidates.first().map(|(rule, _)| *rule);
    }

    match heuristic() {
        Heuristic::First => candidates.first().map(|(rule, _)| *rule),
        Heuristic::All if !candidates.is_empty() => {
            let names: Vec<_> = candidates.iter().map(|(rule, _)| rule.name()).collect();
            candidates
                .get(next_heuristic_all_index(&names))
                .map(|(rule, _)| *rule)
        }
        Heuristic::All => None,
        Heuristic::Random if !candidates.is_empty() => candidates
            .get(next_heuristic_random_index(candidates.len()))
            .map(|(rule, _)| *rule),
        Heuristic::Random => None,
        Heuristic::Compact => candidates
            .iter()
            .min_by_key(|(rule, score)| (*score, rule.name()))
            .map(|(rule, _)| *rule),
    }
}

/// In a comparison operation, it is probably a good idea for the LHS and RHS to
/// have the same representation, if applicable; E.g:
/// ```plain
/// x#MyRepr > y
/// ~>
/// x#MyRepr > y#MyRepr
/// ```
#[register_rule("ReprGeneral", 10100, [Eq, Neq, Lt, Gt, Leq, Geq])]
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
            let new_expr =
                expr.with_children(VecDeque::from([lhs.as_ref().clone(), new_rhs.into()]));
            Ok(Reduction::new(new_expr, constraints, symbols))
        }
        (None, Some((rhs_rule, _))) => {
            let mut new_lhs = lhs_re.clone();
            let (_, symbols, constraints) = new_lhs
                .select_or_init_repr_via(rhs_rule)
                .map_err(|_| RuleNotApplicable)?;
            let new_expr =
                expr.with_children(VecDeque::from([new_lhs.into(), rhs.as_ref().clone()]));
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
            // These domains are concrete for all solvers
            GroundDomain::Bool | GroundDomain::Empty(..) => false,
            // SAT integer encodings remain on the legacy representation path.
            GroundDomain::Int(_) => false,
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

#[cfg(test)]
mod tests {
    use super::*;
    use conjure_cp::ast::{Domain, SetAttr};
    use conjure_cp::settings::set_heuristic;
    use conjure_cp::{domain_int, range};

    #[test]
    fn compact_prefers_the_smallest_representation_domain() {
        set_heuristic(Heuristic::Compact);
        let mut symbols = SymbolTable::new();
        let declaration = symbols.gen_find(&Domain::set(
            SetAttr::new_min_max_size(1, 2),
            domain_int!(1..3),
        ));

        assert_eq!(
            choose_representation_rule(&declaration).unwrap().name(),
            "SetPacked"
        );
        set_heuristic(Heuristic::First);
    }
}
