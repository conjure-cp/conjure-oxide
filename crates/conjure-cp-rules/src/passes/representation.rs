use crate::guard;
use crate::shared::utils::as_cmp_or_lex_op;
use conjure_cp::ast::pretty::pretty_find_with_representation;
use conjure_cp::ast::{Domain, DomainPtr, HasDomain, UnresolvedDomain};
use conjure_cp::settings::{
    Channelling, Heuristic, channelling, heuristic, next_heuristic_all_index,
    next_heuristic_interactive_index, next_heuristic_random_index,
};
use conjure_cp::{
    ast::{Atom, DeclarationPtr, Expression as Expr, GroundDomain, Moo, SymbolTable},
    representation::{ReprRulePtr, get_applicable_repr_by_short_name, get_repr_rules},
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

/// Select a representation named in a `: domain` or `:: type` annotation.
///
/// Example: `1 in (x :: set{packed} of int)` selects the packed representation for this use of `x`.
/// Using different representations at different call sites requires channelling (`--channelling yes`).
#[register_rule("ReprGeneral", 10200, [In, TypeAnnotation, DomainAnnotation])]
fn select_representation_from_annotation(expr: &Expr, _: &SymbolTable) -> ApplicationResult {
    match expr {
        Expr::In(meta, left, right) => {
            let Some((selected, symbols, constraints)) =
                select_repr_from_annotated_expr(right.as_ref())?
            else {
                return Err(RuleNotApplicable);
            };
            Ok(Reduction::new(
                Expr::In(meta.clone(), left.clone(), Moo::new(selected)),
                constraints,
                symbols,
            ))
        }
        Expr::TypeAnnotation(_, _, _) | Expr::DomainAnnotation(_, _, _) => {
            let Some((selected, symbols, constraints)) = select_repr_from_annotated_expr(expr)?
            else {
                return Err(RuleNotApplicable);
            };
            Ok(Reduction::new(selected, constraints, symbols))
        }
        _ => Err(RuleNotApplicable),
    }
}

/// If `expr` is a type/domain annotation that names a representation on a reference, select it.
fn select_repr_from_annotated_expr(
    expr: &Expr,
) -> Result<Option<(Expr, SymbolTable, Vec<Expr>)>, conjure_cp::rule_engine::ApplicationError> {
    let (inner, domain) = match expr {
        Expr::TypeAnnotation(_, inner, domain) | Expr::DomainAnnotation(_, inner, domain) => {
            (inner, domain)
        }
        _ => return Ok(None),
    };

    let Some(pref) = domain.representation_preference() else {
        return Ok(None);
    };

    let Expr::Atomic(_, Atom::Reference(re)) = inner.as_ref() else {
        return Ok(None);
    };

    if let Some(existing) = re.repr
        && existing.short_name() == pref
    {
        return Ok(Some((re.clone().into(), SymbolTable::new(), Vec::new())));
    }

    let Some(rule) = get_applicable_repr_by_short_name(re.ptr(), pref) else {
        return Ok(None);
    };

    let mut re = re.clone();
    let (_, symbols, constraints) = re
        .update_or_init_repr_via(rule)
        .map_err(|_| RuleNotApplicable)?;
    Ok(Some((re.into(), symbols, constraints)))
}

/// Select a representation for abstract domains
#[register_rule("ReprGeneral", 10000, [Atomic / Reference])]
fn select_representation(expr: &Expr, symtab: &SymbolTable) -> ApplicationResult {
    guard!(
        let Expr::Atomic(_, Atom::Reference(re)) = expr &&
        domain_needs_representation(&re.domain_of()) &&
        re.repr.is_none()
        else {
            return Err(RuleNotApplicable)
        }
    );

    let mut re = re.clone();
    let Some(rule) = choose_representation_rule(re.ptr(), symtab) else {
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
    let mut changed = false;
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

        let Some(rule) = choose_representation_rule(decl, &symbols) else {
            continue;
        };
        let mut decl = decl.clone();
        let Ok((new_symbols, new_constraints)) = rule.init_for(&mut decl) else {
            continue;
        };
        symbols.update_insert(decl);
        symbols.extend(new_symbols);
        constraints.extend(new_constraints);
        changed = true;
    }

    // Representation initialisation may introduce no constraints and DeclarationPtr clone-on-write
    // updates are not reliably observable through SymbolTable equality. Record successful
    // initialisation explicitly so zero-constraint layouts (matrix components/packed) still dirty
    // the root and expose their auxiliary declarations.
    if !changed {
        Err(RuleNotApplicable)
    } else {
        Ok(Reduction::new(expr.clone(), constraints, symbols))
    }
}

/// Chooses one applicable representation without mutating the declaration.
///
/// With channelling disabled, declarations that share an identical abstract domain reuse the
/// representation already chosen for that domain. This mirrors Conjure applying one nested
/// representation to a whole matrix-of-sets, rather than letting each matrix component pick
/// independently under the `x` heuristic.
fn choose_representation_rule(decl: &DeclarationPtr, symbols: &SymbolTable) -> Option<ReprRulePtr> {
    if channelling() == Channelling::No
        && let Some(existing) = decl.reprs().iter().next().map(|(_, state)| state.rule())
    {
        return Some(existing);
    }

    if channelling() == Channelling::No
        && let Some(dom) = decl.domain()
        && let Some(rule) = representation_for_identical_domain(decl, &dom, symbols)
    {
        return Some(rule);
    }

    let mut candidates: Vec<_> = get_repr_rules()
        .filter_map(|rule| rule.probe_for(decl).ok().map(|score| (rule, score)))
        .collect();
    candidates.sort_by_key(|(rule, _)| rule.name());

    // User-specified representation preference on the domain (e.g. set{packed}) defaults the choice
    // when that representation is applicable.
    if let Some(dom) = decl.domain()
        && let Some(pref) = dom.representation_preference()
        && let Some((rule, _)) = candidates
            .iter()
            .find(|(rule, _)| rule.short_name() == pref)
    {
        return Some(*rule);
    }

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
        Heuristic::Interactive if !candidates.is_empty() => {
            let labels: Vec<_> = candidates
                .iter()
                .map(|(rule, _)| {
                    pretty_find_with_representation(decl, rule.short_name())
                        .unwrap_or_else(|| rule.name().to_string())
                })
                .collect();
            let label_refs: Vec<_> = labels.iter().map(String::as_str).collect();
            candidates
                .get(next_heuristic_interactive_index(&label_refs))
                .map(|(rule, _)| *rule)
        }
        Heuristic::Interactive => None,
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

/// Find a representation already chosen for another declaration with the same domain.
fn representation_for_identical_domain(
    decl: &DeclarationPtr,
    dom: &DomainPtr,
    symbols: &SymbolTable,
) -> Option<ReprRulePtr> {
    for (name, other) in symbols.iter_local() {
        if *name == *decl.name() {
            continue;
        }
        let Some(other_dom) = other.domain() else {
            continue;
        };
        if &other_dom != dom {
            continue;
        }
        for (_, state) in other.reprs().iter() {
            let rule = state.rule();
            // MatrixComponents is a structural layout, not an abstract-domain representation.
            if rule.name() == "components" {
                continue;
            }
            if rule.probe_for(decl).is_ok() {
                return Some(rule);
            }
        }
    }
    None
}

/// In a comparison operation, it is probably a good idea for the LHS and RHS to
/// have the same representation, if applicable; E.g:
/// ```plain
/// x#MyRepr > y
/// ~>
/// x#MyRepr > y#MyRepr
/// ```
#[register_rule("ReprGeneral", 10100, [Eq, Neq, Lt, Gt, Leq, Geq, LexLt, LexLeq, LexGt, LexGeq])]
fn uniform_repr_in_comparison_op(expr: &Expr, _: &SymbolTable) -> ApplicationResult {
    guard! {
        let Some((lhs, rhs)) = as_cmp_or_lex_op(expr)               &&
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
pub(crate) fn domain_needs_representation(domain: &DomainPtr) -> bool {
    match domain.as_ref() {
        Domain::Ground(gd) => match gd.as_ref() {
            // These domains are concrete for all solvers
            GroundDomain::Empty(..) | GroundDomain::Bool => false,
            // SAT integer encodings remain on the legacy representation path.
            GroundDomain::Int(_) => false,
            // Represent matrices if they have abstract types inside them;
            // Matrices of concrete types are handled separately by the
            // `ReprMatrixComponents`rule set
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
            // `ReprMatrixComponents`rule set
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
            choose_representation_rule(&declaration, &symbols)
                .unwrap()
                .name(),
            "SetPacked"
        );
        set_heuristic(Heuristic::First);
    }

    #[test]
    fn domain_representation_preference_defaults_the_choice() {
        set_heuristic(Heuristic::First);
        let mut symbols = SymbolTable::new();
        let declaration = symbols.gen_find(&Domain::set(
            SetAttr::new_min_max_size(1, 2).with_representation("packed"),
            domain_int!(1..3),
        ));

        assert_eq!(
            choose_representation_rule(&declaration, &symbols)
                .unwrap()
                .short_name(),
            "packed"
        );

        let declaration = symbols.gen_find(&Domain::set(
            SetAttr::new_min_max_size(1, 2).with_representation("explicit"),
            domain_int!(1..3),
        ));

        assert_eq!(
            choose_representation_rule(&declaration, &symbols)
                .unwrap()
                .short_name(),
            "explicit"
        );
    }

    #[test]
    fn annotation_selects_packed_representation() {
        use conjure_cp::ast::{Atom, Expression, Metadata, Moo, Reference};
        use conjure_cp::representation::get_applicable_repr_by_short_name;

        set_heuristic(Heuristic::First);
        let mut symbols = SymbolTable::new();
        let declaration =
            symbols.gen_find(&Domain::set(SetAttr::new_max_size(3), domain_int!(1..4)));
        let re = Reference::new(declaration.clone());
        let annotated = Expression::TypeAnnotation(
            Metadata::new(),
            Moo::new(re.into()),
            Domain::set(
                SetAttr::<i32>::default().with_representation("packed"),
                domain_int!(1..4),
            ),
        );

        let result = select_representation_from_annotation(&annotated, &symbols).unwrap();
        let Expression::Atomic(_, Atom::Reference(selected)) = result.new_expression else {
            panic!("expected a reference, got {}", result.new_expression);
        };
        assert_eq!(selected.get_repr().unwrap().0.short_name(), "packed");
        assert_eq!(
            get_applicable_repr_by_short_name(&declaration, "packed")
                .unwrap()
                .name(),
            "SetPacked"
        );
    }
}
