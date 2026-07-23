use conjure_cp::ast::Moo;
// Equals rule for sets
use conjure_cp::ast::Metadata;
use conjure_cp::ast::{Expression, ReturnType::Set, SymbolTable, Typeable};
use conjure_cp::matrix_expr;
use conjure_cp::rule_engine::RuleEffect;
use conjure_cp::rule_engine::{
    ApplicationError::RuleNotApplicable, ApplicationResult, register_rule,
};

use super::is_set_valued_index;
use Expression::{And, Eq, SubsetEq};

#[register_rule("Base", 8800, [Eq])]
fn eq_to_subset_eq(expr: &Expression, _: &SymbolTable) -> ApplicationResult {
    match expr {
        Eq(_, a, b)
            if matches!(a.as_ref().return_type(), Set(_))
                && matches!(b.as_ref().return_type(), Set(_))
                && !is_set_valued_index(a)
                && !is_set_valued_index(b) =>
        {
            let expr1 = SubsetEq(Metadata::new(), a.clone(), b.clone());
            let expr2 = SubsetEq(Metadata::new(), b.clone(), a.clone());
            Ok(RuleEffect::pure(And(
                Metadata::new(),
                Moo::new(matrix_expr![expr1, expr2]),
            )))
        }
        _ => Err(RuleNotApplicable),
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use conjure_cp::ast::{
        AbstractLiteral, Atom, DeclarationPtr, Domain, Literal, Name, Reference, SetAttr,
    };
    use conjure_cp::{domain_int, range};

    #[test]
    fn set_equality_defers_set_valued_matrix_indices() {
        let matrix = DeclarationPtr::new_find(
            Name::user("sets"),
            Domain::matrix(
                Domain::set(SetAttr::<i32>::default(), domain_int!(1..2)),
                vec![domain_int!(1..2)],
            ),
        );
        let indexed_set = Expression::SafeIndex(
            Metadata::new(),
            Moo::new(Expression::Atomic(
                Metadata::new(),
                Atom::Reference(Reference::new(matrix)),
            )),
            vec![1.into()],
        );
        let empty_set = Literal::AbstractLiteral(AbstractLiteral::Set(vec![])).into();
        let equality = Eq(Metadata::new(), Moo::new(indexed_set), Moo::new(empty_set));

        assert!(matches!(
            eq_to_subset_eq(&equality, &SymbolTable::new()),
            Err(RuleNotApplicable)
        ));
    }
}
