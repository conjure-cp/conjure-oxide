//! Preserve index definedness through library-backed scalar relations.
use conjure_cp::ast::{Expression as Expr, GroundDomain, Metadata, Moo, Range, SymbolTable};
use conjure_cp::rule_engine::{
    ApplicationError::RuleNotApplicable, ApplicationResult, RuleEffect, register_rule,
};

#[register_rule("SAT", 18500, [InDomain])]
fn scalar_membership(expr: &Expr, _: &SymbolTable) -> ApplicationResult {
    let Expr::InDomain(_, value, domain) = expr else {
        return Err(RuleNotApplicable);
    };
    let domain = domain.resolve().map_err(|_| RuleNotApplicable)?;
    let ranges = match domain.as_ref() {
        GroundDomain::Empty(_) => return Ok(RuleEffect::pure(false.into())),
        GroundDomain::Bool if value.domain_of().is_some_and(|domain| domain.is_bool()) => {
            return Ok(RuleEffect::pure(true.into()));
        }
        GroundDomain::Int(ranges, _) if value.domain_of().is_some_and(|domain| domain.is_int()) => {
            ranges
        }
        _ => return Err(RuleNotApplicable),
    };
    let alternatives: Vec<_> = ranges
        .iter()
        .map(|range| {
            let lower =
                |bound| Expr::Geq(Metadata::new(), value.clone(), Moo::new(Expr::from(bound)));
            let upper =
                |bound| Expr::Leq(Metadata::new(), value.clone(), Moo::new(Expr::from(bound)));
            match range {
                Range::Single(bound) => {
                    Expr::Eq(Metadata::new(), value.clone(), Moo::new((*bound).into()))
                }
                Range::Bounded(low, high) => Expr::And(
                    Metadata::new(),
                    Moo::new(conjure_cp::into_matrix_expr!(vec![
                        lower(*low),
                        upper(*high)
                    ])),
                ),
                Range::UnboundedL(high) => upper(*high),
                Range::UnboundedR(low) => lower(*low),
                Range::Unbounded => true.into(),
            }
        })
        .collect();
    Ok(RuleEffect::pure(Expr::Or(
        Metadata::new(),
        Moo::new(conjure_cp::into_matrix_expr!(alternatives)),
    )))
}

#[cfg(test)]
mod tests {
    use super::*;
    use conjure_cp::ast::{Domain, eval_constant};
    #[test]
    fn scalar_membership_keeps_sparse_gaps_and_endpoint_bounds() {
        let domain = Domain::int(vec![Range::Single(-2), Range::Bounded(3, 4)]);
        for value in -3..=5 {
            let expression =
                Expr::InDomain(Metadata::new(), Moo::new(value.into()), domain.clone());
            let effect = scalar_membership(&expression, &SymbolTable::new()).unwrap();
            assert_eq!(
                eval_constant(&effect.new_expression),
                Some((value == -2 || (3..=4).contains(&value)).into())
            );
        }
        for range in [
            Range::UnboundedL(i32::MAX),
            Range::UnboundedR(i32::MIN),
            Range::Unbounded,
        ] {
            let expression = Expr::InDomain(
                Metadata::new(),
                Moo::new(0.into()),
                Domain::int(vec![range]),
            );
            assert_eq!(
                eval_constant(
                    &scalar_membership(&expression, &SymbolTable::new())
                        .unwrap()
                        .new_expression
                ),
                Some(true.into())
            );
        }
    }
}
