//! Select native encodings beneath asserted conjunctions without flattening the worklist.
use conjure_cp::ast::{
    AbstractLiteral, Expression as Expr, Metadata, Moo, SatEncodingDecision, SymbolTable,
};
use conjure_cp::rule_engine::{ApplicationError::RuleNotApplicable, ApplicationResult, RuleEffect};

/// Extract one asserted decision, preserving conjunctions and other Boolean contexts.
pub(super) fn select_asserted(
    expr: &Expr,
    symbols: &SymbolTable,
    select: impl Fn(&Expr) -> Option<SatEncodingDecision>,
) -> ApplicationResult {
    let Expr::Root(_, children) = expr else {
        return Err(RuleNotApplicable);
    };
    for (index, child) in children.iter().enumerate() {
        let Some((replacement, decision)) = select_conjunct(child, &select) else {
            continue;
        };
        let mut children = children.clone();
        if replacement == true.into() {
            children.remove(index);
        } else {
            children[index] = replacement;
        }
        return Ok(RuleEffect::sat(
            Expr::Root(Metadata::new(), children),
            vec![decision],
            symbols.clone(),
        ));
    }
    Err(RuleNotApplicable)
}

fn select_conjunct(
    expr: &Expr,
    select: &impl Fn(&Expr) -> Option<SatEncodingDecision>,
) -> Option<(Expr, SatEncodingDecision)> {
    if let Some(decision) = select(expr) {
        return Some((true.into(), decision));
    }
    let Expr::And(metadata, inner) = expr else {
        return None;
    };
    let Expr::AbstractLiteral(matrix_metadata, AbstractLiteral::Matrix(entries, domain)) =
        inner.as_ref()
    else {
        return None;
    };
    for (index, entry) in entries.iter().enumerate() {
        let Some((replacement, decision)) = select_conjunct(entry, select) else {
            continue;
        };
        let mut entries = entries.clone();
        entries[index] = replacement;
        // Keep the same matrix labels and size; evaluator normalisation removes true later.
        let inner = Expr::AbstractLiteral(
            matrix_metadata.clone(),
            AbstractLiteral::Matrix(entries, domain.clone()),
        );
        return Some((Expr::And(metadata.clone(), Moo::new(inner)), decision));
    }
    None
}
