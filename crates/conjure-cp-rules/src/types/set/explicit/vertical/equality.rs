use crate::guard;
use crate::types::set::explicit::SetExplicit;
use conjure_cp::ast::{AbstractLiteral, Atom, Expression as Expr, Literal, SymbolTable};
use conjure_cp::rule_engine::{
    ApplicationError::RuleNotApplicable, ApplicationResult, RuleEffect, register_rule,
};

/// Lower equality between an explicit set and a set literal.
#[register_rule("ReprGeneral", 9500, [Eq])]
fn eq_explicit_literal(expr: &Expr, _: &SymbolTable) -> ApplicationResult {
    guard!(
        let Expr::Eq(_, lhs, rhs) = expr else {
            return Err(RuleNotApplicable);
        }
    );

    let (set, literal) = match (lhs.as_ref(), rhs.as_ref()) {
        (
            Expr::Atomic(_, Atom::Reference(reference)),
            Expr::Atomic(_, Atom::Literal(Literal::AbstractLiteral(AbstractLiteral::Set(elems)))),
        ) => (reference, elems),
        (
            Expr::Atomic(_, Atom::Literal(Literal::AbstractLiteral(AbstractLiteral::Set(elems)))),
            Expr::Atomic(_, Atom::Reference(reference)),
        ) => (reference, elems),
        _ => return Err(RuleNotApplicable),
    };
    let Some(repr) = set.get_repr_as::<SetExplicit>() else {
        return Err(RuleNotApplicable);
    };

    Ok(RuleEffect::pure(repr.equality_to_literal_expr(literal)))
}

/// Lower equality between two explicit sets.
#[register_rule("ReprGeneral", 9500, [Eq])]
fn eq_explicit(expr: &Expr, _: &SymbolTable) -> ApplicationResult {
    guard!(
        let Expr::Eq(_, lhs, rhs) = expr &&
        let Expr::Atomic(_, Atom::Reference(lhs)) = lhs.as_ref() &&
        let Expr::Atomic(_, Atom::Reference(rhs)) = rhs.as_ref() &&
        let Some(lhs_repr) = lhs.get_repr_as::<SetExplicit>() &&
        let Some(rhs_repr) = rhs.get_repr_as::<SetExplicit>()
        else {
            return Err(RuleNotApplicable);
        }
    );

    Ok(RuleEffect::pure(lhs_repr.equality_expr(&rhs_repr)))
}

#[cfg(test)]
mod tests {
    use super::*;
    use conjure_cp::ast::{Domain, Metadata, Moo, Reference, SetAttr};
    use conjure_cp::representation::ReprRule;
    use conjure_cp::{domain_int, matrix_expr, range};
    use uniplate::Uniplate;

    #[test]
    fn literal_equality_checks_size_and_membership() {
        let domain = Domain::set(SetAttr::new_max_size(2), domain_int!(1..3));
        let mut symbols = SymbolTable::new();
        let mut declaration = symbols.gen_find(&domain);
        SetExplicit::init_for(&mut declaration).unwrap();

        let mut set_reference = Reference::new(declaration);
        let _ = set_reference.select_repr::<SetExplicit>().unwrap();
        let literal = Literal::AbstractLiteral(AbstractLiteral::Set(vec![2.into(), 3.into()]));
        let equality = Expr::Eq(
            Metadata::new(),
            Moo::new(set_reference.clone().into()),
            Moo::new(literal.into()),
        );

        let rewritten = eq_explicit_literal(&equality, &symbols)
            .unwrap()
            .new_expression;
        let nodes = rewritten.universe();
        assert!(matches!(rewritten, Expr::And(..)));
        assert_eq!(
            nodes
                .iter()
                .filter(|node| matches!(node, Expr::Or(..)))
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
        assert_eq!(
            rewritten,
            Expr::And(
                Metadata::new(),
                Moo::new(matrix_expr![
                    Expr::Eq(
                        Metadata::new(),
                        Moo::new(
                            set_reference
                                .get_repr_as::<SetExplicit>()
                                .unwrap()
                                .cardinality_expr()
                        ),
                        Moo::new(2.into()),
                    ),
                    set_reference
                        .get_repr_as::<SetExplicit>()
                        .unwrap()
                        .membership_expr(2.into()),
                    set_reference
                        .get_repr_as::<SetExplicit>()
                        .unwrap()
                        .membership_expr(3.into()),
                ]),
            )
        );
    }

    #[test]
    fn marker_equality_checks_size_and_active_membership() {
        let domain = Domain::set(SetAttr::new_min_size(1), domain_int!(1..2));
        let mut symbols = SymbolTable::new();
        let mut lhs_declaration = symbols.gen_find(&domain);
        let mut rhs_declaration = symbols.gen_find(&domain);
        SetExplicit::init_for(&mut lhs_declaration).unwrap();
        SetExplicit::init_for(&mut rhs_declaration).unwrap();

        let mut lhs = Reference::new(lhs_declaration);
        let mut rhs = Reference::new(rhs_declaration);
        let _ = lhs.select_repr::<SetExplicit>().unwrap();
        let _ = rhs.select_repr::<SetExplicit>().unwrap();
        let equality = Expr::Eq(
            Metadata::new(),
            Moo::new(lhs.clone().into()),
            Moo::new(rhs.clone().into()),
        );

        let rewritten = eq_explicit(&equality, &symbols).unwrap().new_expression;
        let nodes = rewritten.universe();
        assert!(matches!(rewritten, Expr::And(..)));
        assert_eq!(
            nodes
                .iter()
                .filter(|node| matches!(node, Expr::SubsetEq(..) | Expr::Comprehension(..)))
                .count(),
            0
        );
        assert_eq!(
            rewritten,
            lhs.get_repr_as::<SetExplicit>()
                .unwrap()
                .equality_expr(&rhs.get_repr_as::<SetExplicit>().unwrap())
        );
    }
}
