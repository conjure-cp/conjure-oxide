use super::super::SetOccurrence;
use crate::guard;
use conjure_cp::ast::{Atom, Expression as Expr, Reference, SymbolTable, eval_constant};
use conjure_cp::rule_engine::{
    ApplicationError::RuleNotApplicable, ApplicationResult, RuleEffect, register_rule,
};

#[register_rule("ReprGeneral", 9500, [In])]
fn membership_occurrence(expr: &Expr, _: &SymbolTable) -> ApplicationResult {
    guard!(
        let Expr::In(_, member, set) = expr &&
        let Expr::Atomic(_, Atom::Reference(reference)) = set.as_ref() &&
        let Some(repr) = reference.get_repr_as::<SetOccurrence>()
        else { return Err(RuleNotApplicable) }
    );

    if let Some(member) = eval_constant(member) {
        let occurs = repr
            .occurs
            .iter()
            .find(|(value, _)| value.essence_cmp(&member).is_eq())
            .map(|(_, declaration)| Expr::from(Reference::new(declaration.clone())))
            .unwrap_or_else(|| Expr::from(false));
        return Ok(RuleEffect::pure(occurs));
    }

    Ok(RuleEffect::pure(repr.membership_expr((**member).clone())))
}

#[cfg(test)]
mod tests {
    use super::*;
    use conjure_cp::ast::{Domain, Metadata, Moo, SetAttr};
    use conjure_cp::representation::ReprRule;
    use conjure_cp::{domain_int, range};

    fn occurrence_set() -> (SymbolTable, Reference, Vec<conjure_cp::ast::DeclarationPtr>) {
        occurrence_set_over(domain_int!(1..3))
    }

    fn occurrence_set_over(
        inner: conjure_cp::ast::DomainPtr,
    ) -> (SymbolTable, Reference, Vec<conjure_cp::ast::DeclarationPtr>) {
        let domain = Domain::set(SetAttr::<i32>::default(), inner);
        let mut symbols = SymbolTable::new();
        let mut declaration = symbols.gen_find(&domain);
        SetOccurrence::init_for(&mut declaration).unwrap();

        let mut reference = Reference::new(declaration);
        let representation = reference.select_repr::<SetOccurrence>().unwrap().clone();
        let occurs = representation
            .occurs
            .iter()
            .map(|(_, declaration)| declaration.clone())
            .collect();
        (symbols, reference, occurs)
    }

    #[test]
    fn constant_membership_is_the_corresponding_occurrence_bit() {
        let (symbols, set, occurs) = occurrence_set();
        let membership = Expr::In(Metadata::new(), Moo::new(2.into()), Moo::new(set.into()));

        assert_eq!(
            membership_occurrence(&membership, &symbols)
                .unwrap()
                .new_expression,
            Expr::from(Reference::new(occurs[1].clone()))
        );
    }

    /// A variable member must not unroll into one disjunct per inner-domain value: that is
    /// quadratic to rewrite and hides the native `Element` constraint from the Minion backend.
    #[test]
    fn variable_membership_is_an_indexed_lookup() {
        let (symbols, set, occurs) = occurrence_set();
        let member = symbols.clone().gen_find(&domain_int!(1..3));
        let membership = Expr::In(
            Metadata::new(),
            Moo::new(Reference::new(member).into()),
            Moo::new(set.into()),
        );

        let lowered = membership_occurrence(&membership, &symbols)
            .unwrap()
            .new_expression;
        let Expr::SafeIndex(_, subject, indices) = &lowered else {
            panic!("expected an indexed lookup, got {lowered}");
        };
        assert_eq!(indices.len(), 1);
        assert_eq!(subject.unwrap_list().unwrap().len(), occurs.len());
    }

    /// A gappy inner domain has no value-to-position shift, so it keeps the disjunction.
    #[test]
    fn variable_membership_over_a_gappy_domain_stays_a_disjunction() {
        let inner = Domain::int_ground(vec![range!(1), range!(3), range!(5)]);
        let (symbols, set, _) = occurrence_set_over(inner);
        let member = symbols.clone().gen_find(&domain_int!(1..5));
        let membership = Expr::In(
            Metadata::new(),
            Moo::new(Reference::new(member).into()),
            Moo::new(set.into()),
        );

        let lowered = membership_occurrence(&membership, &symbols)
            .unwrap()
            .new_expression;
        assert!(
            matches!(lowered, Expr::Or(..)),
            "expected a disjunction, got {lowered}"
        );
    }

    #[test]
    fn out_of_domain_constant_membership_is_false() {
        let (symbols, set, _) = occurrence_set();
        let membership = Expr::In(Metadata::new(), Moo::new(4.into()), Moo::new(set.into()));

        assert_eq!(
            membership_occurrence(&membership, &symbols)
                .unwrap()
                .new_expression,
            Expr::from(false)
        );
    }
}
