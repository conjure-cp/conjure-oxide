use super::super::SetOccurrence;
use conjure_cp::ast::{Atom, Expression as Expr, Metadata, Moo, Reference, SymbolTable};
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

/// Compare occurrence-set values through Conjure's symmetry-ordering vector.
///
/// Outer explicit-set structural constraints compare adjacent inner values as singleton
/// matrices. Once those values are occurrence-represented, expand `<lex` / `<=lex` to a
/// lexicographic comparison of `[-toInt(bit)]` over the inner domain, matching Conjure's
/// `symmetryOrdering` for `Set_Occurrence`.
#[register_rule("ReprGeneral", 9500, [LexLt, LexLeq])]
fn lex_occurrence_sets(expr: &Expr, _: &SymbolTable) -> ApplicationResult {
    let (lhs, rhs) = match expr {
        Expr::LexLt(_, lhs, rhs) | Expr::LexLeq(_, lhs, rhs) => (lhs, rhs),
        _ => return Err(RuleNotApplicable),
    };

    let lhs_reference = singleton_reference(lhs).ok_or(RuleNotApplicable)?;
    let rhs_reference = singleton_reference(rhs).ok_or(RuleNotApplicable)?;
    let lhs_repr = lhs_reference
        .get_repr_as::<SetOccurrence>()
        .ok_or(RuleNotApplicable)?;
    let rhs_repr = rhs_reference
        .get_repr_as::<SetOccurrence>()
        .ok_or(RuleNotApplicable)?;

    let lhs_order = lhs_repr.symmetry_ordering_expr();
    let rhs_order = rhs_repr.symmetry_ordering_expr();
    let rewritten = match expr {
        Expr::LexLt(..) => Expr::LexLt(Metadata::new(), Moo::new(lhs_order), Moo::new(rhs_order)),
        Expr::LexLeq(..) => Expr::LexLeq(Metadata::new(), Moo::new(lhs_order), Moo::new(rhs_order)),
        _ => unreachable!(),
    };
    Ok(RuleEffect::pure(rewritten))
}

#[cfg(test)]
mod tests {
    use super::*;
    use conjure_cp::ast::{Domain, SetAttr};
    use conjure_cp::matrix_expr;
    use conjure_cp::representation::ReprRule;
    use conjure_cp::{domain_int, range};
    use uniplate::Uniplate;

    #[test]
    fn occurrence_set_lex_order_uses_negated_to_int_bits() {
        let domain = Domain::set(SetAttr::new_max_size(2), domain_int!(1..2));
        let mut symbols = SymbolTable::new();
        let mut lhs_declaration = symbols.gen_find(&domain);
        let mut rhs_declaration = symbols.gen_find(&domain);
        SetOccurrence::init_for(&mut lhs_declaration).unwrap();
        SetOccurrence::init_for(&mut rhs_declaration).unwrap();

        let mut lhs = Reference::new(lhs_declaration);
        let mut rhs = Reference::new(rhs_declaration);
        let _ = lhs.select_repr::<SetOccurrence>().unwrap();
        let _ = rhs.select_repr::<SetOccurrence>().unwrap();
        let comparison = Expr::LexLt(
            Metadata::new(),
            Moo::new(matrix_expr![Expr::from(lhs)]),
            Moo::new(matrix_expr![Expr::from(rhs)]),
        );

        let rewritten = lex_occurrence_sets(&comparison, &symbols)
            .unwrap()
            .new_expression;
        let nodes = rewritten.universe();
        assert!(matches!(rewritten, Expr::LexLt(..)));
        assert_eq!(
            nodes
                .iter()
                .filter(|node| matches!(node, Expr::Neg(..)))
                .count(),
            4
        );
        assert_eq!(
            nodes
                .iter()
                .filter(|node| matches!(node, Expr::ToInt(..)))
                .count(),
            4
        );
    }
}
