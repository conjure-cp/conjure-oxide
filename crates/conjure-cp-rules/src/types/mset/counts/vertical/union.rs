use super::super::MSetCounts;
use conjure_cp::ast::{
    AbstractLiteral, Atom, Expression as Expr, Literal, Metadata, Moo, SymbolTable,
};
use conjure_cp::rule_engine::{
    ApplicationError::RuleNotApplicable, ApplicationResult, RuleEffect, register_rule,
};
use conjure_cp::{into_matrix_expr, matrix_expr};

/// Candidate members of a multiset expression paired with their slot counts.
fn support(expr: &Expr) -> Option<Vec<(Expr, Expr)>> {
    match expr {
        Expr::Atomic(_, Atom::Reference(reference)) => {
            let state = reference.get_repr_as::<MSetCounts>()?;
            Some(
                (1..=state.max_distinct)
                    .map(|index| (state.value_expr(index), state.count_expr(index)))
                    .collect(),
            )
        }
        Expr::Atomic(_, Atom::Literal(Literal::AbstractLiteral(AbstractLiteral::MSet(elems)))) => {
            Some(
                elems
                    .iter()
                    .cloned()
                    .map(Expr::from)
                    .map(|value| (value, 1.into()))
                    .collect(),
            )
        }
        Expr::AbstractLiteral(_, AbstractLiteral::MSet(elems)) => Some(
            elems
                .iter()
                .cloned()
                .map(|value| (value, 1.into()))
                .collect(),
        ),
        Expr::Union(_, lhs, rhs) => {
            let mut values = support(lhs)?;
            values.extend(support(rhs)?);
            Some(values)
        }
        _ => None,
    }
}

fn sum(terms: Vec<Expr>) -> Expr {
    match terms.as_slice() {
        [] => 0.into(),
        [term] => term.clone(),
        _ => Expr::Sum(Metadata::new(), Moo::new(into_matrix_expr!(terms))),
    }
}

fn frequency(support: &[(Expr, Expr)], member: Expr) -> Expr {
    sum(support
        .iter()
        .map(|(value, count)| {
            let matches = Expr::ToInt(
                Metadata::new(),
                Moo::new(Expr::Eq(
                    Metadata::new(),
                    Moo::new(value.clone()),
                    Moo::new(member.clone()),
                )),
            );
            Expr::Product(
                Metadata::new(),
                Moo::new(matrix_expr![count.clone(), matches]),
            )
        })
        .collect())
}

fn equal_frequency_if_active(
    member: &Expr,
    count: &Expr,
    result: &[(Expr, Expr)],
    source: &[(Expr, Expr)],
) -> Expr {
    Expr::Or(
        Metadata::new(),
        Moo::new(matrix_expr![
            Expr::Eq(Metadata::new(), Moo::new(count.clone()), Moo::new(0.into())),
            Expr::Eq(
                Metadata::new(),
                Moo::new(frequency(result, member.clone())),
                Moo::new(frequency(source, member.clone()))
            )
        ]),
    )
}

/// Channel a counts-represented auxiliary to the value-level union which created it.
///
/// Every value with non-zero frequency occurs in at least one active result/source slot. Checking
/// both finite supports is therefore equivalent to checking the entire element domain, without
/// enumerating that domain.
#[register_rule("ReprGeneral", 9500, [AuxDeclaration / Union])]
fn union_counts(expr: &Expr, _: &SymbolTable) -> ApplicationResult {
    let Expr::AuxDeclaration(_, result_reference, source) = expr else {
        return Err(RuleNotApplicable);
    };
    let Expr::Union(..) = source.as_ref() else {
        return Err(RuleNotApplicable);
    };
    if result_reference.get_repr_as::<MSetCounts>().is_none() {
        return Err(RuleNotApplicable);
    }

    // Representation selection tags the auxiliary result reference itself; preserve that exact
    // reference rather than reconstructing representation state from its declaration.
    let result_expr = Expr::from(result_reference.clone());
    let result = support(&result_expr).ok_or(RuleNotApplicable)?;
    let source = support(source).ok_or(RuleNotApplicable)?;
    let constraints = result
        .iter()
        .chain(&source)
        .map(|(member, count)| equal_frequency_if_active(member, count, &result, &source))
        .collect::<Vec<_>>();

    Ok(RuleEffect::pure(Expr::And(
        Metadata::new(),
        Moo::new(into_matrix_expr!(constraints)),
    )))
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::shared::utils::to_aux_var;
    use conjure_cp::ast::{Domain, MSetAttr, Reference};
    use conjure_cp::representation::ReprRule;
    use conjure_cp::{domain_int, range};
    use uniplate::Uniplate;

    #[test]
    fn union_auxiliary_is_linked_over_finite_slot_supports() {
        let domain = Domain::mset(MSetAttr::new_max_size(2), domain_int!(1..999));
        let mut symbols = SymbolTable::new();
        let mut result = symbols.gen_find_auxiliary(&domain);
        let mut operand = symbols.gen_find_auxiliary(&domain);
        MSetCounts::init_for(&mut result).unwrap();
        MSetCounts::init_for(&mut operand).unwrap();

        let counts_reference = |ptr| Reference {
            ptr,
            repr: Some(MSetCounts::STORED),
        };
        let source = Expr::Union(
            Metadata::new(),
            Moo::new(Expr::from(counts_reference(operand))),
            Moo::new(Expr::AbstractLiteral(
                Metadata::new(),
                AbstractLiteral::MSet(vec![1.into(), 2.into()]),
            )),
        );
        let auxiliary =
            Expr::AuxDeclaration(Metadata::new(), counts_reference(result), Moo::new(source));

        let rewritten = union_counts(&auxiliary, &symbols).unwrap().new_expression;
        let universe = rewritten.universe();
        assert_eq!(
            universe
                .iter()
                .filter(|expr| matches!(expr, Expr::Or(..)))
                .count(),
            6
        );
        assert!(
            universe
                .iter()
                .all(|expr| !matches!(expr, Expr::Union(..) | Expr::AuxDeclaration(..)))
        );
    }

    #[test]
    fn union_auxiliary_inherits_the_selected_counts_representation() {
        let domain = Domain::mset(MSetAttr::new_max_size(2), domain_int!(1..999));
        let mut symbols = SymbolTable::new();
        let mut operand = symbols.gen_find_auxiliary(&domain);
        MSetCounts::init_for(&mut operand).unwrap();
        symbols.update_insert(operand.clone());

        let source = Expr::Union(
            Metadata::new(),
            Moo::new(Expr::from(Reference {
                ptr: operand,
                repr: Some(MSetCounts::STORED),
            })),
            Moo::new(Expr::AbstractLiteral(
                Metadata::new(),
                AbstractLiteral::MSet(vec![1.into(), 2.into()]),
            )),
        );

        let auxiliary = to_aux_var(&source, &symbols).unwrap();
        let Atom::Reference(reference) = auxiliary.as_atom() else {
            panic!("expected an auxiliary reference");
        };
        assert_eq!(reference.get_repr().unwrap().0.short_name(), "counts");
        assert!(
            auxiliary
                .top_level_expr()
                .universe()
                .iter()
                .any(|expr| matches!(expr, Expr::AuxDeclaration(_, reference, _) if reference.repr == Some(MSetCounts::STORED)))
        );
    }
}
