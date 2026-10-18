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
