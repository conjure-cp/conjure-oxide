//! Lifts Conjure's attribute-as-constraint predicate syntax (`reflexive(r)`, `size(s, 3)`, ...)
//! into the declaration domain attributes of its target, when the target is a plain reference to
//! a locally declared `find`/`given` and the attribute can be losslessly merged into the domain.
//! Mirrors Conjure's `attributeAsConstraints_OnStmts` (`Process/AttributeAsConstraints.hs`).
//!
//! Only [`GroundDomain::Set`], [`GroundDomain::MSet`], [`GroundDomain::Function`] and
//! [`GroundDomain::Relation`] are covered, matching every domain kind Conjure's own
//! `attributeToConstraint` dispatch table supports (it has no case for `DomainSequence`, which
//! does not exist in vanilla Conjure -- Oxide's `Sequence` is its own campaign type with no
//! Conjure AAC equivalent to port). `Partition` is excluded because it has not been started as an
//! Oxide campaign type at all: there is no domain/representation support to hang partition-specific
//! attributes off of.
//!
//! Anything that cannot be safely lifted (a symbolic value, an unresolved domain, a target that
//! isn't a bare top-level reference to a `find`/`given`, or an attribute/domain-kind combination
//! this does not cover) is left in place for the fallback expansion rule
//! ([`crate::passes::attribute_as_constraint_fallback`]) to turn into an equivalent formula.

use std::collections::HashMap;

use conjure_cp::{
    ast::{
        Atom, BinaryAttr, DecisionVariable, DeclarationKind, DeclarationPtr, Domain, DomainPtr,
        Expression as Expr, GroundDomain, JectivityAttr, Literal as Lit, Moo, Name, PartialityAttr,
        Range, SymbolTable, eval_constant,
    },
    rule_engine::{ApplicationError, ApplicationResult, RuleEffect, register_rule},
};

use ApplicationError::RuleNotApplicable;

type Int = i32;

/// Lifts every top-level `AttributeAsConstraint` whose target can be merged into a declaration
/// domain, dropping it from the constraint list and updating the declaration in its place.
#[register_rule("Base", 10300, [Root])]
fn lift_attribute_as_constraints(expr: &Expr, symbols: &SymbolTable) -> ApplicationResult {
    let Expr::Root(meta, children) = expr else {
        return Err(RuleNotApplicable);
    };

    let mut kept_children = Vec::with_capacity(children.len());
    // Keyed by declaration name so several AACs targeting the same declaration (e.g.
    // `minSize(s, 1)` and `maxSize(s, 3)`) narrow the same domain in sequence rather than
    // clobbering each other.
    let mut updated: HashMap<Name, (DeclarationPtr, DomainPtr)> = HashMap::new();

    for child in children {
        let Expr::AttributeAsConstraint(_, target, attr, value) = child else {
            kept_children.push(child.clone());
            continue;
        };

        match try_lift(target, attr.as_str(), value.as_deref(), symbols, &updated) {
            Some((decl, new_domain)) => {
                let name = decl.name().clone();
                updated.insert(name, (decl, new_domain));
            }
            None => kept_children.push(child.clone()),
        }
    }

    if updated.is_empty() {
        return Err(RuleNotApplicable);
    }

    let declaration_updates: Vec<(DeclarationPtr, DeclarationKind)> = updated
        .into_values()
        .map(|(decl, new_domain)| {
            let updated_kind = match &*decl.kind() {
                DeclarationKind::Find(_) => {
                    DeclarationKind::Find(DecisionVariable::new(new_domain))
                }
                DeclarationKind::FindAuxiliary(_) => {
                    DeclarationKind::FindAuxiliary(DecisionVariable::new(new_domain))
                }
                DeclarationKind::Given(_) => DeclarationKind::Given(new_domain),
                other => conjure_cp::bug!(
                    "try_lift only returns Find/FindAuxiliary/Given declarations, got {other:?}"
                ),
            };
            (decl, updated_kind)
        })
        .collect();

    Ok(RuleEffect::new(
        Expr::Root(meta.clone(), kept_children),
        vec![],
        SymbolTable::new(),
    )
    .with_declaration_updates(declaration_updates))
}

/// Attempts to merge one `AttributeAsConstraint` into its target's declaration domain, returning
/// the declaration and its updated domain on success.
fn try_lift(
    target: &Expr,
    attr_name: &str,
    value: Option<&Expr>,
    symbols: &SymbolTable,
    already_updated: &HashMap<Name, (DeclarationPtr, DomainPtr)>,
) -> Option<(DeclarationPtr, DomainPtr)> {
    let Expr::Atomic(_, Atom::Reference(reference)) = target else {
        return None;
    };

    let name = reference.name().clone();
    let decl = symbols.lookup_local(&name)?;

    if !matches!(
        &*decl.kind(),
        DeclarationKind::Find(_) | DeclarationKind::FindAuxiliary(_) | DeclarationKind::Given(_)
    ) {
        return None;
    }

    let current_domain = match already_updated.get(&name) {
        Some((_, dom)) => dom.clone(),
        None => decl.domain()?,
    };

    let Domain::Ground(ground) = current_domain.as_ref() else {
        // Unresolved domains (e.g. a size bound referring to another variable) cannot be safely
        // narrowed here; fall back to expanding the AAC to an explicit formula instead.
        return None;
    };

    let value = match value {
        Some(value_expr) => match eval_constant(value_expr)? {
            Lit::Int(v) => Some(v),
            _ => return None,
        },
        None => None,
    };

    let new_ground = merge_attribute_into_ground_domain(ground.as_ref(), attr_name, value)?;
    let new_domain = Moo::new(Domain::Ground(Moo::new(new_ground)));
    Some((decl, new_domain))
}

/// Merges one attribute into a ground domain's attributes, returning `None` if the attribute does
/// not apply to this domain kind, or if merging it would conflict with an existing bound (in which
/// case the caller should fall back to expanding the AAC as an explicit, possibly-unsatisfiable,
/// constraint rather than silently dropping information).
fn merge_attribute_into_ground_domain(
    domain: &GroundDomain,
    attr_name: &str,
    value: Option<Int>,
) -> Option<GroundDomain> {
    match domain {
        GroundDomain::Set(attr, inner) => {
            let mut attr = attr.clone();
            attr.size = merge_size_range(&attr.size, attr_name, value?)?;
            Some(GroundDomain::Set(attr, inner.clone()))
        }
        GroundDomain::MSet(attr, inner) => {
            let mut attr = attr.clone();
            match attr_name {
                "size" | "minSize" | "maxSize" => {
                    attr.size = merge_size_range(&attr.size, attr_name, value?)?;
                }
                "minOccur" | "maxOccur" => {
                    attr.occurrence = merge_size_range(&attr.occurrence, attr_name, value?)?;
                }
                _ => return None,
            }
            Some(GroundDomain::MSet(attr, inner.clone()))
        }
        GroundDomain::Function(attr, dom, cdom) => {
            let mut attr = attr.clone();
            match attr_name {
                "size" | "minSize" | "maxSize" => {
                    attr.size = merge_size_range(&attr.size, attr_name, value?)?;
                }
                "total" => attr.partiality = PartialityAttr::Total,
                "injective" | "surjective" | "bijective" => {
                    attr.jectivity = merge_jectivity(&attr.jectivity, attr_name)?;
                }
                _ => return None,
            }
            Some(GroundDomain::Function(attr, dom.clone(), cdom.clone()))
        }
        GroundDomain::Relation(attr, inners) => {
            let mut attr = attr.clone();
            match attr_name {
                "size" | "minSize" | "maxSize" => {
                    attr.size = merge_size_range(&attr.size, attr_name, value?)?;
                }
                _ => {
                    // Binary attributes only make sense (and are only ever validated by Conjure)
                    // on arity-2 relations.
                    let bin_attr = BinaryAttr::from_keyword(attr_name)?;
                    if inners.len() != 2 {
                        return None;
                    }
                    if !attr.binary.contains(&bin_attr) {
                        attr.binary.push(bin_attr);
                    }
                }
            }
            Some(GroundDomain::Relation(attr, inners.clone()))
        }
        _ => None,
    }
}

/// Narrows a `size`/`minSize`/`maxSize`-shaped range attribute, returning `None` if the new bound
/// would conflict with the existing one (so the caller falls back to an explicit, and in that case
/// necessarily unsatisfiable, constraint rather than silently discarding the conflict).
fn merge_size_range(current: &Range<Int>, kind: &str, value: Int) -> Option<Range<Int>> {
    let mut lo = current.low().copied();
    let mut hi = current.high().copied();
    match kind {
        "size" => {
            if !current.contains(&value) {
                return None;
            }
            lo = Some(value);
            hi = Some(value);
        }
        "minSize" | "minOccur" => {
            lo = Some(lo.map_or(value, |l| l.max(value)));
        }
        "maxSize" | "maxOccur" => {
            hi = Some(hi.map_or(value, |h| h.min(value)));
        }
        _ => return None,
    }
    if let (Some(l), Some(h)) = (lo, hi)
        && l > h
    {
        return None;
    }
    Some(Range::new(lo, hi))
}

/// Narrows a function's jectivity attribute, returning `None` for a genuine conflict (e.g.
/// requesting `injective` on a domain already narrowed some other, incompatible way -- there is
/// none today since `None`/`Injective`/`Surjective`/`Bijective` only ever combine towards
/// `Bijective`, but the explicit `None` keeps this total if that ever changes).
fn merge_jectivity(current: &JectivityAttr, attr_name: &str) -> Option<JectivityAttr> {
    let requested = match attr_name {
        "injective" => JectivityAttr::Injective,
        "surjective" => JectivityAttr::Surjective,
        "bijective" => JectivityAttr::Bijective,
        _ => return None,
    };
    Some(match (current, &requested) {
        (JectivityAttr::None, _) => requested,
        (c, r) if *c == *r => requested,
        (JectivityAttr::Bijective, _) | (_, JectivityAttr::Bijective) => JectivityAttr::Bijective,
        (JectivityAttr::Injective, JectivityAttr::Surjective)
        | (JectivityAttr::Surjective, JectivityAttr::Injective) => JectivityAttr::Bijective,
        #[allow(unreachable_patterns)]
        _ => return None,
    })
}

#[cfg(test)]
mod tests {
    use super::*;
    use conjure_cp::ast::{DeclarationPtr, Metadata, Reference, RelAttr, SetAttr};
    use conjure_cp::rule_engine::get_rule_by_name;
    use conjure_cp::{domain_int, range};
    use ustr::Ustr;

    fn root(children: Vec<Expr>) -> Expr {
        Expr::Root(Metadata::new(), children)
    }

    fn attr_expr(target: Expr, attr: &str, value: Option<Expr>) -> Expr {
        Expr::AttributeAsConstraint(
            Metadata::new(),
            Moo::new(target),
            Ustr::from(attr),
            value.map(Moo::new),
        )
    }

    fn ref_expr(decl: &DeclarationPtr) -> Expr {
        Expr::Atomic(
            Metadata::new(),
            Atom::Reference(Reference::new(decl.clone())),
        )
    }

    fn int_lit(v: i32) -> Expr {
        Expr::Atomic(Metadata::new(), Atom::Literal(Lit::Int(v)))
    }

    fn rule() -> &'static conjure_cp::rule_engine::Rule<'static> {
        get_rule_by_name("lift_attribute_as_constraints").expect("rule registered")
    }

    #[test]
    fn lifts_size_into_set_attr() {
        let mut symbols = SymbolTable::new();
        let s = DeclarationPtr::new_find(
            Name::user("s"),
            Domain::set(SetAttr::<Int>::default(), domain_int!(1..5)),
        );
        symbols.insert(s.clone()).expect("insert s");

        let tree = root(vec![attr_expr(ref_expr(&s), "size", Some(int_lit(3)))]);
        let result = rule().apply(&tree, &symbols).expect("should lift size");

        let Expr::Root(_, kept) = &result.new_expression else {
            panic!("expected Root")
        };
        assert!(
            kept.is_empty(),
            "the lifted AAC should be removed from the constraint list"
        );
        assert_eq!(
            result.updated_declaration_names().collect::<Vec<_>>(),
            vec![Name::user("s")]
        );
    }

    #[test]
    fn lifts_minsize_and_maxsize_from_two_separate_aacs_on_the_same_declaration() {
        let mut symbols = SymbolTable::new();
        let s = DeclarationPtr::new_find(
            Name::user("s"),
            Domain::set(SetAttr::<Int>::default(), domain_int!(1..5)),
        );
        symbols.insert(s.clone()).expect("insert s");

        let tree = root(vec![
            attr_expr(ref_expr(&s), "minSize", Some(int_lit(2))),
            attr_expr(ref_expr(&s), "maxSize", Some(int_lit(4))),
        ]);
        let result = rule().apply(&tree, &symbols).expect("should lift both");

        let Expr::Root(_, kept) = &result.new_expression else {
            panic!("expected Root")
        };
        assert!(kept.is_empty());
        // Only one declaration update should be recorded for `s`, reflecting both narrowings.
        assert_eq!(
            result.updated_declaration_names().collect::<Vec<_>>(),
            vec![Name::user("s")]
        );
    }

    #[test]
    fn declines_when_target_is_not_a_bare_reference() {
        let symbols = SymbolTable::new();
        let non_reference_target = Expr::Atomic(Metadata::new(), Atom::Literal(Lit::Bool(true)));
        let tree = root(vec![attr_expr(
            non_reference_target,
            "size",
            Some(int_lit(3)),
        )]);

        let err = rule().apply(&tree, &symbols).unwrap_err();
        assert!(matches!(err, ApplicationError::RuleNotApplicable));
    }

    #[test]
    fn declines_when_value_is_not_a_constant() {
        let mut symbols = SymbolTable::new();
        let bound = DeclarationPtr::new_find(Name::user("n"), domain_int!(1..5));
        symbols.insert(bound.clone()).expect("insert n");
        let s = DeclarationPtr::new_find(
            Name::user("s"),
            Domain::set(SetAttr::<Int>::default(), domain_int!(1..5)),
        );
        symbols.insert(s.clone()).expect("insert s");

        let tree = root(vec![attr_expr(
            ref_expr(&s),
            "size",
            Some(ref_expr(&bound)),
        )]);
        let err = rule().apply(&tree, &symbols).unwrap_err();
        assert!(matches!(err, ApplicationError::RuleNotApplicable));
    }

    #[test]
    fn lifts_reflexive_into_relation_binary_attrs() {
        let mut symbols = SymbolTable::new();
        let r = DeclarationPtr::new_find(
            Name::user("r"),
            Domain::relation(
                RelAttr::<Int>::default(),
                vec![domain_int!(1..3), domain_int!(1..3)],
            ),
        );
        symbols.insert(r.clone()).expect("insert r");

        let tree = root(vec![attr_expr(ref_expr(&r), "reflexive", None)]);
        let result = rule()
            .apply(&tree, &symbols)
            .expect("should lift reflexive");

        let Expr::Root(_, kept) = &result.new_expression else {
            panic!("expected Root")
        };
        assert!(kept.is_empty());
    }

    #[test]
    fn declines_binary_attr_lift_when_relation_arity_is_not_two() {
        let mut symbols = SymbolTable::new();
        let r = DeclarationPtr::new_find(
            Name::user("r"),
            Domain::relation(
                RelAttr::<Int>::default(),
                vec![domain_int!(1..3), domain_int!(1..3), domain_int!(1..3)],
            ),
        );
        symbols.insert(r.clone()).expect("insert r");

        let tree = root(vec![attr_expr(ref_expr(&r), "reflexive", None)]);
        let err = rule().apply(&tree, &symbols).unwrap_err();
        assert!(matches!(err, ApplicationError::RuleNotApplicable));
    }
}
