use crate::types::set::explicit::SetExplicit;
use conjure_cp::ast::{Atom, Expression as Expr, Metadata, Moo, Reference, SymbolTable};
use conjure_cp::matrix_expr;
use conjure_cp::rule_engine::{
    ApplicationError::RuleNotApplicable, ApplicationResult, RuleEffect, register_rule,
};

fn singleton_reference(expr: &Expr) -> Option<Reference> {
    let values = expr.unwrap_list()?;
    let [Expr::Atomic(_, Atom::Reference(reference))] = values.as_slice() else {
        return None;
    };
    Some(reference.clone())
}

/// Compare explicit-set values through their concrete representation.
///
/// Outer explicit-set structural constraints compare adjacent inner values as singleton matrices.
/// Once matrix indexing exposes those values as represented set references, expand the comparison
/// to marker-first lexicographic ordering followed by the represented element matrices. This is the
/// same vertical lowering Conjure applies for explicit-over-explicit sets.
#[register_rule("ReprGeneral", 9500, [LexLt, LexLeq])]
fn lex_explicit_sets(expr: &Expr, _: &SymbolTable) -> ApplicationResult {
    let (lhs, rhs) = match expr {
        Expr::LexLt(_, lhs, rhs) | Expr::LexLeq(_, lhs, rhs) => (lhs, rhs),
        _ => return Err(RuleNotApplicable),
    };

    let lhs_reference = singleton_reference(lhs).ok_or(RuleNotApplicable)?;
    let rhs_reference = singleton_reference(rhs).ok_or(RuleNotApplicable)?;
    let lhs_repr = lhs_reference
        .get_repr_as::<SetExplicit>()
        .ok_or(RuleNotApplicable)?;
    let rhs_repr = rhs_reference
        .get_repr_as::<SetExplicit>()
        .ok_or(RuleNotApplicable)?;

    let lhs_size: Expr = Reference::new(lhs_repr.set_size.clone()).into();
    let rhs_size: Expr = Reference::new(rhs_repr.set_size.clone()).into();
    let lhs_values = Expr::Flatten(
        Metadata::new(),
        None,
        Moo::new(Reference::new(lhs_repr.elems_matrix.clone()).into()),
    );
    let rhs_values = Expr::Flatten(
        Metadata::new(),
        None,
        Moo::new(Reference::new(rhs_repr.elems_matrix.clone()).into()),
    );
    let compare_values = match expr {
        Expr::LexLt(..) => Expr::LexLt(Metadata::new(), Moo::new(lhs_values), Moo::new(rhs_values)),
        Expr::LexLeq(..) => {
            Expr::LexLeq(Metadata::new(), Moo::new(lhs_values), Moo::new(rhs_values))
        }
        _ => unreachable!(),
    };

    Ok(RuleEffect::pure(Expr::Or(
        Metadata::new(),
        Moo::new(matrix_expr![
            Expr::Lt(
                Metadata::new(),
                Moo::new(lhs_size.clone()),
                Moo::new(rhs_size.clone())
            ),
            Expr::And(
                Metadata::new(),
                Moo::new(matrix_expr![
                    Expr::Eq(Metadata::new(), Moo::new(lhs_size), Moo::new(rhs_size)),
                    compare_values,
                ])
            )
        ]),
    )))
}

#[cfg(test)]
mod tests {
    use super::*;
    use conjure_cp::ast::{Domain, SetAttr};
    use conjure_cp::representation::ReprRule;
    use conjure_cp::{domain_int, matrix_expr, range};
    use uniplate::Uniplate;

    #[test]
    fn explicit_set_lex_order_uses_marker_then_values() {
        let domain = Domain::set(SetAttr::new_max_size(2), domain_int!(1..2));
        let mut symbols = SymbolTable::new();
        let mut lhs_declaration = symbols.gen_find(&domain);
        let mut rhs_declaration = symbols.gen_find(&domain);
        SetExplicit::init_for(&mut lhs_declaration).unwrap();
        SetExplicit::init_for(&mut rhs_declaration).unwrap();

        let mut lhs = Reference::new(lhs_declaration);
        let mut rhs = Reference::new(rhs_declaration);
        let _ = lhs.select_repr::<SetExplicit>().unwrap();
        let _ = rhs.select_repr::<SetExplicit>().unwrap();
        let comparison = Expr::LexLt(
            Metadata::new(),
            Moo::new(matrix_expr![Expr::from(lhs)]),
            Moo::new(matrix_expr![Expr::from(rhs)]),
        );

        let rewritten = lex_explicit_sets(&comparison, &symbols)
            .unwrap()
            .new_expression;
        let nodes = rewritten.universe();
        assert!(matches!(rewritten, Expr::Or(..)));
        assert_eq!(
            nodes
                .iter()
                .filter(|node| matches!(node, Expr::Lt(..)))
                .count(),
            1
        );
        assert_eq!(
            nodes
                .iter()
                .filter(|node| matches!(node, Expr::Flatten(..)))
                .count(),
            2
        );
        assert_eq!(
            nodes
                .iter()
                .filter(|node| matches!(node, Expr::LexLt(..)))
                .count(),
            1
        );
    }
}
