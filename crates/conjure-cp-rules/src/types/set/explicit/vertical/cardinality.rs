use crate::guard;
use crate::types::set::explicit::SetExplicit;
use conjure_cp::ast::{Atom, Expression as Expr, SymbolTable};
use conjure_cp::rule_engine::{
    ApplicationError::RuleNotApplicable, ApplicationResult, RuleEffect, register_rule,
};

/// Lower explicit-set cardinality to its marker or fixed size.
#[register_rule("ReprGeneral", 9500, [Card])]
fn cardinality_explicit(expr: &Expr, _: &SymbolTable) -> ApplicationResult {
    guard!(
        let Expr::Card(_, set) = expr &&
        let Expr::Atomic(_, Atom::Reference(reference)) = set.as_ref() &&
        let Some(repr) = reference.get_repr_as::<SetExplicit>()
        else {
            return Err(RuleNotApplicable);
        }
    );

    Ok(RuleEffect::pure(repr.cardinality_expr()))
}

#[cfg(test)]
mod tests {
    use super::*;
    use conjure_cp::ast::{Domain, Metadata, Moo, Reference, SetAttr};
    use conjure_cp::representation::ReprRule;
    use conjure_cp::{domain_int, range};

    #[test]
    fn variable_cardinality_is_the_marker_reference() {
        let domain = Domain::set(SetAttr::new_max_size(2), domain_int!(1..2));
        let mut symbols = SymbolTable::new();
        let mut declaration = symbols.gen_find(&domain);
        SetExplicit::init_for(&mut declaration).unwrap();

        let mut set_reference = Reference::new(declaration);
        let repr = set_reference.select_repr::<SetExplicit>().unwrap().clone();
        let cardinality = Expr::Card(Metadata::new(), Moo::new(set_reference.into()));

        let rewritten = cardinality_explicit(&cardinality, &symbols)
            .unwrap()
            .new_expression;
        let Expr::Atomic(_, Atom::Reference(marker)) = rewritten else {
            panic!("expected the marker reference");
        };
        assert_eq!(Some(marker.ptr()), repr.set_size.as_ref());
    }

    #[test]
    fn fixed_cardinality_is_a_literal() {
        let domain = Domain::set(SetAttr::new_size(2), domain_int!(1..3));
        let mut symbols = SymbolTable::new();
        let mut declaration = symbols.gen_find(&domain);
        SetExplicit::init_for(&mut declaration).unwrap();

        let mut set_reference = Reference::new(declaration);
        let repr = set_reference.select_repr::<SetExplicit>().unwrap().clone();
        let cardinality = Expr::Card(Metadata::new(), Moo::new(set_reference.into()));

        let rewritten = cardinality_explicit(&cardinality, &symbols)
            .unwrap()
            .new_expression;
        assert_eq!(rewritten, Expr::from(2));
        assert!(repr.set_size.is_none());
    }
}
