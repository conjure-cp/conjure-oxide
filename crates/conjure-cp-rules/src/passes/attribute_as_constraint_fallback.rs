//! Expands any `AttributeAsConstraint` node that [`crate::passes::attribute_as_constraint`]
//! could not lift into its target's declaration domain, into an equivalent formula built from
//! ordinary operators. Mirrors Conjure's `mkAttributeToConstraint` (`Language/EvaluateOp.hs`),
//! which produces the same formula the lifting path would have merged, for exactly the same
//! reason: a bare `attr(x)`/`attr(x, val)` predicate can appear anywhere a bool expression can,
//! not only as a bare top-level constraint.
//!
//! Coverage is narrower than Conjure's own dispatch table in one respect: `size`/`minSize`/
//! `maxSize`/`minOccur`/`maxOccur` expand via [`Expression::Card`], and binary relation attributes
//! expand via the same [`binary_relation_constraints`] helper every relation representation's own
//! structural constraints already share -- both routes only work because `Card` and `In` already
//! have rules for every relevant representation (Set, MSet, Relation). Function jectivity/
//! totality and MSet's per-element occurrence bound have no such representation-independent
//! realisation in this codebase (`Image`/`Defined` have no rules for any function representation,
//! and there is no generic "count of x in mset" operator), so those combinations are only usable
//! via the lifting path; if lifting declines them too (e.g. because the target isn't a bare
//! top-level reference), the model fails to translate rather than being silently wrong.

use conjure_cp::{
    ast::{
        Atom, BinaryAttr, Domain, DomainPtr, Expression as Expr, GroundDomain, Literal as Lit,
        Metadata, Moo, UnresolvedDomain,
    },
    into_matrix_expr,
    rule_engine::{ApplicationError, ApplicationResult, RuleEffect, register_rule},
};

use crate::types::relation::binary_attrs::{binary_relation_constraints, tuple2};

use ApplicationError::RuleNotApplicable;

enum SizeCmp {
    Eq,
    Geq,
    Leq,
}

fn size_cmp(attr_name: &str) -> Option<SizeCmp> {
    match attr_name {
        "size" => Some(SizeCmp::Eq),
        "minSize" | "minOccur" => Some(SizeCmp::Geq),
        "maxSize" | "maxOccur" => Some(SizeCmp::Leq),
        _ => None,
    }
}

/// Expands one un-lifted `AttributeAsConstraint` node in place.
#[register_rule("Base", 9800, [AttributeAsConstraint])]
fn expand_attribute_as_constraint(
    expr: &Expr,
    _symbols: &conjure_cp::ast::SymbolTable,
) -> ApplicationResult {
    let Expr::AttributeAsConstraint(_, target, attr, value) = expr else {
        return Err(RuleNotApplicable);
    };
    let attr_name = attr.as_str();

    if let Some(cmp) = size_cmp(attr_name) {
        let value_expr = value.clone().ok_or(RuleNotApplicable)?;
        let card = Moo::new(Expr::Card(Metadata::new(), target.clone()));
        let formula = match cmp {
            SizeCmp::Eq => Expr::Eq(Metadata::new(), card, value_expr),
            SizeCmp::Geq => Expr::Geq(Metadata::new(), card, value_expr),
            SizeCmp::Leq => Expr::Leq(Metadata::new(), card, value_expr),
        };
        return Ok(RuleEffect::pure(formula));
    }

    if let Some(bin_attr) = BinaryAttr::from_keyword(attr_name) {
        let domain = target.as_ref().domain_of().ok_or(RuleNotApplicable)?;
        let col_domain = binary_relation_column_domain(&domain).ok_or(RuleNotApplicable)?;
        let target = target.clone();
        let member = move |x: &Expr, y: &Expr| {
            Expr::In(Metadata::new(), Moo::new(tuple2(x, y)), target.clone())
        };

        let constraints =
            binary_relation_constraints(&col_domain, std::slice::from_ref(&bin_attr), &member);
        let formula = match constraints.len() {
            0 => Expr::Atomic(Metadata::new(), Atom::Literal(Lit::Bool(true))),
            1 => constraints.into_iter().next().expect("checked len == 1"),
            _ => Expr::And(Metadata::new(), Moo::new(into_matrix_expr!(constraints))),
        };
        return Ok(RuleEffect::pure(formula));
    }

    Err(RuleNotApplicable)
}

/// Returns the shared column domain of a binary (arity-2, equal-column) relation domain, or
/// `None` if `domain` isn't shaped like one -- binary relation attributes are only ever validated
/// (by Conjure and this campaign alike) on relations whose two columns share a domain.
fn binary_relation_column_domain(domain: &DomainPtr) -> Option<DomainPtr> {
    match domain.as_ref() {
        Domain::Ground(g) => match g.as_ref() {
            GroundDomain::Relation(_, inners) if inners.len() == 2 && inners[0] == inners[1] => {
                Some(Moo::new(Domain::Ground(inners[0].clone())))
            }
            _ => None,
        },
        Domain::Unresolved(u) => match u.as_ref() {
            UnresolvedDomain::Relation(_, inners)
                if inners.len() == 2 && inners[0] == inners[1] =>
            {
                Some(inners[0].clone())
            }
            _ => None,
        },
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use conjure_cp::ast::{DeclarationPtr, Name, Reference, RelAttr, SetAttr, SymbolTable};
    use conjure_cp::rule_engine::get_rule_by_name;
    use conjure_cp::{domain_int, range};
    use ustr::Ustr;

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
        get_rule_by_name("expand_attribute_as_constraint").expect("rule registered")
    }

    #[test]
    fn expands_size_into_a_card_equality() {
        let mut symbols = SymbolTable::new();
        let s = DeclarationPtr::new_find(
            Name::user("s"),
            Domain::set(SetAttr::<i32>::default(), domain_int!(1..5)),
        );
        symbols.insert(s.clone()).expect("insert s");

        let expr = attr_expr(ref_expr(&s), "size", Some(int_lit(3)));
        let result = rule().apply(&expr, &symbols).expect("should expand size");
        assert!(matches!(result.new_expression, Expr::Eq(_, _, _)));
    }

    #[test]
    fn expands_minsize_into_a_card_geq_and_maxsize_into_a_card_leq() {
        let mut symbols = SymbolTable::new();
        let s = DeclarationPtr::new_find(
            Name::user("s"),
            Domain::set(SetAttr::<i32>::default(), domain_int!(1..5)),
        );
        symbols.insert(s.clone()).expect("insert s");

        let min_result = rule()
            .apply(
                &attr_expr(ref_expr(&s), "minSize", Some(int_lit(2))),
                &symbols,
            )
            .expect("should expand minSize");
        assert!(matches!(min_result.new_expression, Expr::Geq(_, _, _)));

        let max_result = rule()
            .apply(
                &attr_expr(ref_expr(&s), "maxSize", Some(int_lit(4))),
                &symbols,
            )
            .expect("should expand maxSize");
        assert!(matches!(max_result.new_expression, Expr::Leq(_, _, _)));
    }

    #[test]
    fn expands_reflexive_into_a_quantified_formula() {
        let mut symbols = SymbolTable::new();
        let r = DeclarationPtr::new_find(
            Name::user("r"),
            Domain::relation(
                RelAttr::<i32>::default(),
                vec![domain_int!(1..3), domain_int!(1..3)],
            ),
        );
        symbols.insert(r.clone()).expect("insert r");

        let expr = attr_expr(ref_expr(&r), "reflexive", None);
        let result = rule()
            .apply(&expr, &symbols)
            .expect("should expand reflexive");
        assert!(matches!(result.new_expression, Expr::And(_, _)));
    }

    #[test]
    fn declines_binary_attr_expansion_when_relation_arity_is_not_two() {
        let mut symbols = SymbolTable::new();
        let r = DeclarationPtr::new_find(
            Name::user("r"),
            Domain::relation(
                RelAttr::<i32>::default(),
                vec![domain_int!(1..3), domain_int!(1..3), domain_int!(1..3)],
            ),
        );
        symbols.insert(r.clone()).expect("insert r");

        let expr = attr_expr(ref_expr(&r), "reflexive", None);
        let err = rule().apply(&expr, &symbols).unwrap_err();
        assert!(matches!(err, ApplicationError::RuleNotApplicable));
    }

    #[test]
    fn declines_when_no_expansion_route_exists() {
        let mut symbols = SymbolTable::new();
        let f = DeclarationPtr::new_find(
            Name::user("f"),
            conjure_cp::ast::Domain::function(
                conjure_cp::ast::FuncAttr::<i32>::default(),
                domain_int!(1..3),
                domain_int!(1..3),
            ),
        );
        symbols.insert(f.clone()).expect("insert f");

        let expr = attr_expr(ref_expr(&f), "injective", None);
        let err = rule().apply(&expr, &symbols).unwrap_err();
        assert!(matches!(err, ApplicationError::RuleNotApplicable));
    }
}
