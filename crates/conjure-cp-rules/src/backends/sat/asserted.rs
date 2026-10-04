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

#[cfg(test)]
mod tests {
    use super::*;
    use conjure_cp::ast::{Atom, DeclarationPtr, Domain, Literal, Name, Reference, eval_constant};
    use conjure_cp::into_matrix_expr;
    use uniplate::Uniplate;

    fn variable(name: &str) -> Expr {
        Reference::new(DeclarationPtr::new_find(Name::user(name), Domain::bool())).into()
    }

    fn conjunction(entries: Vec<Expr>) -> Expr {
        Expr::And(Metadata::new(), Moo::new(into_matrix_expr!(entries)))
    }

    fn evaluate(expr: &Expr, p: bool, q: bool) -> bool {
        let expr = if let Expr::Root(_, entries) = expr {
            conjunction(entries.clone())
        } else {
            expr.clone()
        };
        let expr = expr.transform(&|expr| match expr {
            Expr::Atomic(_, Atom::Reference(reference)) => {
                if *reference.name() == Name::user("p") {
                    p.into()
                } else {
                    q.into()
                }
            }
            other => other,
        });
        eval_constant(&expr) == Some(Literal::Bool(true))
    }

    #[test]
    fn extracting_an_asserted_conjunct_preserves_projection() {
        let p = variable("p");
        let q = variable("q");
        let guarded = Expr::Or(
            Metadata::new(),
            Moo::new(into_matrix_expr!(vec![
                q.clone(),
                Expr::Not(Metadata::new(), Moo::new(p.clone())),
            ])),
        );
        let root = Expr::Root(
            Metadata::new(),
            vec![conjunction(vec![guarded, conjunction(vec![p.clone(), q])])],
        );
        let effect = select_asserted(&root, &SymbolTable::new(), |expr| {
            (expr == &p).then(|| SatEncodingDecision::Assert(p.clone()))
        })
        .unwrap();
        let [SatEncodingDecision::Assert(assertion)] = effect.new_sat_decisions.as_slice() else {
            panic!("expected one assertion")
        };
        for p in [false, true] {
            for q in [false, true] {
                assert_eq!(
                    evaluate(&root, p, q),
                    evaluate(&effect.new_expression, p, q) && evaluate(assertion, p, q)
                );
            }
        }
    }

    #[test]
    fn guarded_negated_and_reified_conjuncts_are_not_assertions() {
        let p = variable("p");
        let q = variable("q");
        for context in [
            Expr::Or(
                Metadata::new(),
                Moo::new(into_matrix_expr!(vec![p.clone(), q.clone()])),
            ),
            Expr::Not(Metadata::new(), Moo::new(p.clone())),
            Expr::Imply(Metadata::new(), Moo::new(q.clone()), Moo::new(p.clone())),
            Expr::Iff(Metadata::new(), Moo::new(q), Moo::new(p.clone())),
        ] {
            let root = Expr::Root(Metadata::new(), vec![conjunction(vec![context])]);
            assert!(
                select_asserted(&root, &SymbolTable::new(), |expr| {
                    (expr == &p).then(|| SatEncodingDecision::Assert(p.clone()))
                })
                .is_err()
            );
        }
    }
}
