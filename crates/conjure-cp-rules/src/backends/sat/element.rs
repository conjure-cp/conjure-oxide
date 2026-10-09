//! Retain safe lookup definitions until library clause generation.
use conjure_cp::ast::{
    Expression as Expr, GroundDomain, Metadata, Moo, Reference, SatEncodingDecision, SymbolTable,
};
use conjure_cp::rule_engine::{
    ApplicationError::RuleNotApplicable, ApplicationResult, RuleEffect, register_rule,
};

/// Materialise scalar matrix entries with their actual numeric index labels.
pub(super) fn matrix_data(subject: &Expr) -> Option<(Vec<Expr>, Vec<i64>)> {
    let subject = crate::shared::utils::table_operand(subject);
    let list = subject.is_list();
    let (entries, domain) = subject.unwrap_matrix_unchecked()?;
    let labels = if list {
        (1..=entries.len())
            .map(|value| i64::try_from(value).ok())
            .collect::<Option<Vec<_>>>()?
    } else {
        let domain = domain.resolve().ok()?;
        match domain.as_ref() {
            GroundDomain::Bool => vec![0, 1],
            domain => domain
                .values_i32()
                .ok()?
                .into_iter()
                .map(i64::from)
                .collect(),
        }
    };
    (entries.len() == labels.len()).then_some((entries, labels))
}

/// Hoist a total safe value; bubbling continues to govern undefined unsafe expressions.
#[register_rule("SAT", 18400, [SafeIndex])]
fn introduce_element(expr: &Expr, symbols: &SymbolTable) -> ApplicationResult {
    let Expr::SafeIndex(_, subject, indices) = expr else {
        return Err(RuleNotApplicable);
    };
    if indices.len() != 1 || conjure_cp::ast::eval_constant(&indices[0]).is_some() {
        return Err(RuleNotApplicable);
    }
    let (entries, _) = matrix_data(subject).ok_or(RuleNotApplicable)?;
    if entries.is_empty() {
        return Err(RuleNotApplicable);
    }
    let domain = expr.domain_of().ok_or(RuleNotApplicable)?;
    let mut symbols = symbols.clone();
    let value = Reference::new(symbols.gen_find_auxiliary(&domain));
    let value: Expr = value.into();
    let definition = Expr::SatElement(
        Metadata::new(),
        subject.clone(),
        Moo::new(indices[0].clone()),
        Moo::new(value.clone()),
    );
    Ok(RuleEffect::new(value, vec![definition], symbols))
}

/// Select whole-value equalities through the same element algorithms as scalar values.
#[register_rule("SAT", 18400, [SatElement])]
fn introduce_compound_element(expr: &Expr, symbols: &SymbolTable) -> ApplicationResult {
    let Expr::SatElement(_, subject, index, value) = expr else {
        return Err(RuleNotApplicable);
    };
    let domain = value.domain_of().ok_or(RuleNotApplicable)?;
    if domain.is_int() || domain.is_bool() {
        return Err(RuleNotApplicable);
    }
    let (entries, labels) = matrix_data(subject).ok_or(RuleNotApplicable)?;
    let mut symbols = symbols.clone();
    let mut definitions = Vec::new();
    let comparisons = entries
        .into_iter()
        .map(|entry| {
            let output = super::boolean::create_bool_aux(&mut symbols);
            definitions.push(Expr::Iff(
                Metadata::new(),
                Moo::new(output.clone()),
                Moo::new(Expr::Eq(Metadata::new(), value.clone(), Moo::new(entry))),
            ));
            output
        })
        .collect();
    let labels = labels
        .into_iter()
        .map(|label| i32::try_from(label).map(conjure_cp::ast::Range::Single))
        .collect::<Result<Vec<_>, _>>()
        .map_err(|_| RuleNotApplicable)?;
    let comparisons =
        conjure_cp::into_matrix_expr![comparisons; conjure_cp::ast::Domain::int(labels)];
    Ok(RuleEffect::new(
        Expr::SatElement(
            Metadata::new(),
            Moo::new(comparisons),
            index.clone(),
            Moo::new(true.into()),
        ),
        definitions,
        symbols,
    ))
}

/// Numeric views retain actual sparse values and physical representation invariants.
#[register_rule("SAT", 18500, [SatElement])]
fn select_element(expr: &Expr, symbols: &SymbolTable) -> ApplicationResult {
    let Expr::SatElement(_, subject, index, value) = expr else {
        return Err(RuleNotApplicable);
    };
    let (entries, labels) = matrix_data(subject).ok_or(RuleNotApplicable)?;
    let index = super::pseudo_boolean::integer_view(index).ok_or(RuleNotApplicable)?;
    let value = super::pseudo_boolean::integer_view(value).ok_or(RuleNotApplicable)?;
    let entries = entries
        .iter()
        .zip(labels)
        .map(|(entry, label)| super::pseudo_boolean::integer_view(entry).map(|view| (label, view)))
        .collect::<Option<Vec<_>>>()
        .ok_or(RuleNotApplicable)?;
    Ok(RuleEffect::sat(
        true.into(),
        vec![SatEncodingDecision::Element {
            index_view: index,
            value,
            entries,
            encoding: None,
            pb_encoding: None,
        }],
        symbols.clone(),
    ))
}

#[cfg(test)]
mod tests {
    use super::*;
    use conjure_cp::ast::{AbstractLiteral, DeclarationPtr, Domain, Name, Range};

    #[test]
    fn element_preserves_actual_sparse_labels_and_declines_constant_indices() {
        let matrix = Expr::AbstractLiteral(
            Metadata::new(),
            AbstractLiteral::Matrix(
                vec![(-5).into(), 7.into()],
                Domain::int(vec![Range::Single(-2), Range::Single(3)]),
            ),
        );
        assert_eq!(matrix_data(&matrix).unwrap().1, vec![-2, 3]);
        let index: Expr = Reference::new(DeclarationPtr::new_find(
            Name::User("index".into()),
            Domain::int(vec![Range::Bounded(-2, 3)]),
        ))
        .into();
        let safe = Expr::SafeIndex(
            Metadata::new(),
            Moo::new(matrix.clone()),
            vec![index.clone()],
        );
        let effect = introduce_element(&safe, &SymbolTable::new()).unwrap();
        assert!(matches!(effect.new_top[0], Expr::SatElement(_, _, _, _)));
        assert!(introduce_element(&effect.new_top[0], &SymbolTable::new()).is_err());
        let definition = Expr::SatElement(
            Metadata::new(),
            Moo::new(matrix),
            Moo::new((-2).into()),
            Moo::new(7.into()),
        );
        let effect = select_element(&definition, &SymbolTable::new()).unwrap();
        assert!(
            matches!(&effect.new_sat_decisions[0], SatEncodingDecision::Element { entries, .. } if entries.iter().map(|(label, _)| *label).collect::<Vec<_>>() == vec![-2, 3])
        );
        let constant = Expr::SafeIndex(
            Metadata::new(),
            Moo::new(conjure_cp::into_matrix_expr!(vec![1.into(), 2.into()])),
            vec![1.into()],
        );
        assert!(introduce_element(&constant, &SymbolTable::new()).is_err());
    }

    #[test]
    fn boolean_matrix_indices_keep_zero_and_one() {
        let matrix = Expr::AbstractLiteral(
            Metadata::new(),
            AbstractLiteral::Matrix(vec![true.into(), false.into()], Domain::bool()),
        );
        assert_eq!(matrix_data(&matrix).unwrap().1, vec![0, 1]);
    }

    #[test]
    fn compound_lookup_keeps_sparse_labels_and_whole_value_equalities() {
        let tuple_domain =
            Domain::tuple(vec![Domain::bool(), Domain::int(vec![Range::Single(-2)])]);
        let value: Expr = Reference::new(DeclarationPtr::new_find(
            Name::User("tuple".into()),
            tuple_domain,
        ))
        .into();
        let entry = Expr::AbstractLiteral(
            Metadata::new(),
            AbstractLiteral::Tuple(vec![false.into(), (-2).into()]),
        );
        let matrix =
            conjure_cp::into_matrix_expr![vec![entry.clone()]; Domain::int(vec![Range::Single(3)])];
        let definition = Expr::SatElement(
            Metadata::new(),
            Moo::new(matrix),
            Moo::new(3.into()),
            Moo::new(value.clone()),
        );
        let effect = introduce_compound_element(&definition, &SymbolTable::new()).unwrap();
        let Expr::SatElement(_, comparisons, _, selected) = &effect.new_expression else {
            panic!("expected a Boolean element definition");
        };
        assert_eq!(**selected, Expr::from(true));
        assert_eq!(matrix_data(comparisons).unwrap().1, vec![3]);
        assert!(
            matches!(&effect.new_top[0], Expr::Iff(_, _, equality) if matches!(equality.as_ref(), Expr::Eq(_, left, right) if **left == value && **right == entry))
        );
    }
}
