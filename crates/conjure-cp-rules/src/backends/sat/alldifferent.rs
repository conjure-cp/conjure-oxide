//! Retain allDifferent as a semantic decision until library clause generation.
use conjure_cp::ast::{Expression as Expr, SatEncodingDecision, SymbolTable};
use conjure_cp::rule_engine::{
    ApplicationError::RuleNotApplicable, ApplicationResult, RuleEffect, register_rule,
};

/// Preserve ready numeric operands, including nested Boolean uses.
#[register_rule("SAT", 18500, [AllDiff])]
fn select_alldifferent(expr: &Expr, symbols: &SymbolTable) -> ApplicationResult {
    let Expr::AllDiff(_, matrix) = expr else {
        return Err(RuleNotApplicable);
    };
    let elements = matrix.unwrap_list_cow().ok_or(RuleNotApplicable)?;
    let inputs = elements
        .iter()
        .map(super::pseudo_boolean::integer_view)
        .collect::<Option<Vec<_>>>()
        .ok_or(RuleNotApplicable)?;
    let mut symbols = symbols.clone();
    let output = super::boolean::create_bool_aux(&mut symbols);
    Ok(RuleEffect::sat(
        output.clone(),
        vec![SatEncodingDecision::AllDifferent {
            output,
            inputs,
            encoding: None,
            amo_encoding: None,
            pb_encoding: None,
        }],
        symbols,
    ))
}

#[cfg(test)]
mod tests {
    use super::*;
    use conjure_cp::ast::{
        AbstractLiteral, DeclarationPtr, Domain, Metadata, Moo, Name, Reference, SATIntEncoding,
    };

    #[test]
    fn alldifferent_retains_actual_numeric_views_and_declines_compound_values() {
        let bit: Expr = Reference::new(DeclarationPtr::new_find(
            Name::User("bit".into()),
            Domain::bool(),
        ))
        .into();
        let matrix = |inputs| {
            Expr::AbstractLiteral(
                Metadata::new(),
                AbstractLiteral::matrix_implied_indices(inputs),
            )
        };
        let direct = Expr::SATInt(
            Metadata::new(),
            SATIntEncoding::Direct,
            Moo::new(matrix(vec![
                bit.clone(),
                Expr::Not(Metadata::new(), Moo::new(bit.clone())),
            ])),
            (-1, 0),
        );
        let input = Expr::AllDiff(Metadata::new(), Moo::new(matrix(vec![direct, 0.into()])));
        let effect = select_alldifferent(&input, &SymbolTable::new()).unwrap();
        assert!(
            matches!(&effect.new_sat_decisions[0], SatEncodingDecision::AllDifferent { inputs, encoding: None, .. }
            if inputs[0].choices.as_ref().unwrap().iter().map(|(value, _)| *value).collect::<Vec<_>>() == vec![-1, 0]
                && inputs[1].constant == 0)
        );
        let constant = Expr::SATInt(
            Metadata::new(),
            SATIntEncoding::Log,
            Moo::new(matrix(vec![Expr::from(false)])),
            (42, 42),
        );
        let view = super::super::pseudo_boolean::integer_view(&constant).unwrap();
        assert_eq!(view.constant, 0, "Read literal bits, not the bounds");
        assert!(view.choices.is_some());
        let compound = Expr::AllDiff(
            Metadata::new(),
            Moo::new(matrix(vec![matrix(vec![bit.clone()]), matrix(vec![bit])])),
        );
        assert!(select_alldifferent(&compound, &SymbolTable::new()).is_err());
    }
}
