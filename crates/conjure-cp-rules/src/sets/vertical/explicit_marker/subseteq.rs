use crate::guard;
use crate::representation::SetExplicitVarSizeWithMarker;
use conjure_cp::ast::{Atom, Expression as Expr, SymbolTable};
use conjure_cp::rule_engine::{
    ApplicationError::RuleNotApplicable, ApplicationResult, RuleEffect, register_rule,
};

/// Lower subset inclusion from an explicit marker set.
#[register_rule("ReprGeneral", 9500, [SubsetEq])]
fn subseteq_explicit_marker(expr: &Expr, _: &SymbolTable) -> ApplicationResult {
    guard!(
        let Expr::SubsetEq(_, lhs, rhs) = expr &&
        let Expr::Atomic(_, Atom::Reference(lhs)) = lhs.as_ref() &&
        let Some(lhs_repr) = lhs.get_repr_as::<SetExplicitVarSizeWithMarker>()
        else {
            return Err(RuleNotApplicable);
        }
    );

    Ok(RuleEffect::pure(lhs_repr.subset_expr((**rhs).clone())))
}

#[cfg(test)]
mod tests {
    use super::*;
    use conjure_cp::ast::{AbstractLiteral, Domain, Literal, Metadata, Moo, Reference, SetAttr};
    use conjure_cp::representation::ReprRule;
    use conjure_cp::{domain_int, range};
    use uniplate::Uniplate;

    #[test]
    fn subset_checks_only_active_marker_slots() {
        let domain = Domain::set(SetAttr::new_max_size(2), domain_int!(1..3));
        let mut symbols = SymbolTable::new();
        let mut declaration = symbols.gen_find(&domain);
        SetExplicitVarSizeWithMarker::init_for(&mut declaration).unwrap();

        let mut set_reference = Reference::new(declaration);
        let _ = set_reference
            .select_repr::<SetExplicitVarSizeWithMarker>()
            .unwrap();
        let literal = Literal::AbstractLiteral(AbstractLiteral::Set(vec![1.into(), 2.into()]));
        let subset = Expr::SubsetEq(
            Metadata::new(),
            Moo::new(set_reference.clone().into()),
            Moo::new(literal.clone().into()),
        );

        let rewritten = subseteq_explicit_marker(&subset, &symbols)
            .unwrap()
            .new_expression;
        let nodes = rewritten.universe();
        assert_eq!(
            rewritten,
            set_reference
                .get_repr_as::<SetExplicitVarSizeWithMarker>()
                .unwrap()
                .subset_expr(literal.into())
        );
        assert_eq!(
            nodes
                .iter()
                .filter(|node| matches!(node, Expr::In(..)))
                .count(),
            2
        );
        assert_eq!(
            nodes
                .iter()
                .filter(|node| matches!(node, Expr::SubsetEq(..) | Expr::Comprehension(..)))
                .count(),
            0
        );
    }
}
