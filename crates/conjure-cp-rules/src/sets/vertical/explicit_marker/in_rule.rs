use crate::guard;
use crate::representation::SetExplicitVarSizeWithMarker;
use conjure_cp::ast::{Atom, Expression as Expr, SymbolTable};
use conjure_cp::rule_engine::{
    ApplicationError::RuleNotApplicable, ApplicationResult, RuleEffect, register_rule,
};

/// Lower membership in an explicit marker set to active-slot equality checks.
#[register_rule("ReprGeneral", 9500, [In])]
fn in_explicit_marker(expr: &Expr, _: &SymbolTable) -> ApplicationResult {
    guard!(
        let Expr::In(_, member, set) = expr &&
        let Expr::Atomic(_, Atom::Reference(reference)) = set.as_ref() &&
        let Some(repr) = reference.get_repr_as::<SetExplicitVarSizeWithMarker>()
        else {
            return Err(RuleNotApplicable);
        }
    );

    Ok(RuleEffect::pure(repr.membership_expr((**member).clone())))
}

#[cfg(test)]
mod tests {
    use super::*;
    use conjure_cp::ast::{Domain, Metadata, Moo, Reference, SetAttr};
    use conjure_cp::representation::ReprRule;
    use conjure_cp::{domain_int, range};
    use uniplate::Uniplate;

    #[test]
    fn membership_checks_only_active_marker_slots() {
        let domain = Domain::set(SetAttr::new_max_size(2), domain_int!(1..2));
        let mut symbols = SymbolTable::new();
        let mut declaration = symbols.gen_find(&domain);
        SetExplicitVarSizeWithMarker::init_for(&mut declaration).unwrap();

        let mut set_reference = Reference::new(declaration);
        let _ = set_reference
            .select_repr::<SetExplicitVarSizeWithMarker>()
            .unwrap();
        let membership = Expr::In(
            Metadata::new(),
            Moo::new(1.into()),
            Moo::new(set_reference.into()),
        );

        let rewritten = in_explicit_marker(&membership, &symbols)
            .unwrap()
            .new_expression;
        let nodes = rewritten.universe();
        assert!(matches!(rewritten, Expr::Or(..)));
        assert_eq!(
            nodes
                .iter()
                .filter(|node| matches!(node, Expr::And(..)))
                .count(),
            2
        );
        assert_eq!(
            nodes
                .iter()
                .filter(|node| matches!(node, Expr::SafeIndex(..)))
                .count(),
            2
        );
        assert_eq!(
            nodes
                .iter()
                .filter(|node| matches!(node, Expr::Leq(..)))
                .count(),
            2
        );
    }
}
