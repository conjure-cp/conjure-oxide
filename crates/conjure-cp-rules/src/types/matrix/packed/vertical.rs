use super::MatrixPacked;
use conjure_cp::ast::{Atom, Expression as Expr, Metadata, Moo, SymbolTable};
use conjure_cp::rule_engine::{
    ApplicationError::RuleNotApplicable, ApplicationResult, RuleEffect, register_rule,
    register_rule_set,
};

// Whole-matrix mixed-radix decoding can be expensive for indexing-heavy models, but that is a
// trade-off for the heuristic to weigh, not something to settle by disabling the rules: a rule set
// is enabled by which solver is being targeted and nothing else. Packing a matrix into a single
// integer works for any solver with integers, so this is on everywhere, and `MatrixPacked`
// competes with `MatrixComponents` at the same choice site.
register_rule_set!("ReprMatrixPacked", ("Base"), |_| true);

/// A packed matrix is semantically transparent: decode it to an ordinary indexed matrix
/// expression, after which the representation-independent matrix rules apply.
#[register_rule("ReprMatrixPacked", 9800, [Atomic / Reference])]
fn decode_packed_matrix_reference(expr: &Expr, _: &SymbolTable) -> ApplicationResult {
    let Expr::Atomic(_, Atom::Reference(reference)) = expr else {
        return Err(RuleNotApplicable);
    };
    let Some(representation) = reference.get_repr_as::<MatrixPacked>() else {
        return Err(RuleNotApplicable);
    };
    Ok(RuleEffect::pure(representation.decoded_matrix()))
}

/// Preserve matrix symmetry ordering without materialising every decoded element.
#[register_rule("ReprMatrixPacked", 9900, [LexLt, LexLeq])]
fn order_packed_matrices(expr: &Expr, _: &SymbolTable) -> ApplicationResult {
    let (lhs, rhs) = match expr {
        Expr::LexLt(_, lhs, rhs) | Expr::LexLeq(_, lhs, rhs) => (lhs, rhs),
        _ => return Err(RuleNotApplicable),
    };
    let [
        Expr::Atomic(_, Atom::Reference(lhs)),
        Expr::Atomic(_, Atom::Reference(rhs)),
    ] = [lhs.as_ref(), rhs.as_ref()]
    else {
        return Err(RuleNotApplicable);
    };
    let (Some(lhs), Some(rhs)) = (
        lhs.get_repr_as::<MatrixPacked>(),
        rhs.get_repr_as::<MatrixPacked>(),
    ) else {
        return Err(RuleNotApplicable);
    };
    if lhs.values != rhs.values || lhs.dimensions != rhs.dimensions {
        return Err(RuleNotApplicable);
    }
    let (lhs, rhs) = (Moo::new(lhs.packed_expr()), Moo::new(rhs.packed_expr()));
    Ok(RuleEffect::pure(match expr {
        Expr::LexLt(..) => Expr::Lt(Metadata::new(), lhs, rhs),
        Expr::LexLeq(..) => Expr::Leq(Metadata::new(), lhs, rhs),
        _ => unreachable!(),
    }))
}

#[cfg(test)]
mod tests {
    #[test]
    fn packed_matrix_rules_are_registered() {
        // Packed matrices are reached through representation selection, like every other layout,
        // rather than by a sweep of their own; what this rule set has to provide is the decoding
        // that makes a selected packed matrix behave like an ordinary indexed one.
        let rules = conjure_cp::rule_engine::get_rule_set_by_name("ReprMatrixPacked")
            .unwrap()
            .get_rules();
        assert!(
            rules
                .keys()
                .any(|rule| rule.name == "decode_packed_matrix_reference")
        );
    }
}
