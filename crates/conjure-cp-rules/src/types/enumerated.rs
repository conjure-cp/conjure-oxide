//! Represent enum members by their declaration-order indices.
use crate::shared::representation_prelude::*;
use crate::shared::utils::as_eq_or_neq;
use conjure_cp::ast::enumerated::{EnumVariant, EnumeratedType};
use conjure_cp::ast::{
    Domain, GroundDomain, Moo, Range, Reference, ReturnType, Typeable, eval_constant,
};
use conjure_cp::rule_engine::{RuleEffect, register_rule};

register_representation!(
    EnumAsInt("integer")
    struct State<T> {
        pub ty: Moo<EnumeratedType>,
        pub value: T
    }
    fn init(dom: DomainPtr) -> Result<State<DomainPtr>, ReprInitError> {
        let ground = dom.resolve().map_err(|_| ReprInitError::UnsupportedDomain(dom.clone(), EnumAsInt::NAME, "expected a ground enum domain".into()))?;
        let GroundDomain::EnumeratedType(ty, ranges) = ground.as_ref() else {
            return Err(ReprInitError::UnsupportedDomain(dom, EnumAsInt::NAME, "expected an enum domain".into()));
        };
        let Ok(size) = i32::try_from(ty.len()) else {
            return Err(ReprInitError::UnsupportedDomain(dom, EnumAsInt::NAME, "enum type is too large".into()));
        };
        let last = size - 1;
        let ranges = if ranges.is_empty() && size > 0 {
            vec![Range::Bounded(0, last)]
        } else {
            ranges.iter().filter_map(|range| {
                let low = i32::try_from(*range.low().unwrap_or(&0)).ok()?;
                let high = range.high().and_then(|&high| i32::try_from(high).ok()).map_or(last, |high| high.min(last));
                (low <= high).then(|| Range::new(Some(low), Some(high)))
            }).collect()
        };
        Ok(State { ty: ty.clone(), value: Domain::int_ground(ranges) })
    }
    fn structural(_state: &State<DeclarationPtr>) -> Vec<Expression> { vec![] }
    fn down(state: &State<DomainPtr>, value: Literal) -> Result<State<Literal>, ReprDownError> {
        if let Literal::EnumVariant(member) = &value
            && member.ty == state.ty
            && let Ok(index) = i32::try_from(member.variant)
            && state.value.contains(&Literal::Int(index)) == Ok(true)
        {
            return Ok(State { ty: state.ty.clone(), value: Literal::Int(index) });
        }
        Err(ReprDownError::BadValue(value, "expected a member of the enum domain".into()))
    }
    fn up(state: State<Literal>) -> Literal {
        let Literal::Int(value) = state.value else { bug!("expected an integer enum member") };
        Literal::EnumVariant(EnumVariant::new(state.ty, value as u32))
    }
);

/// Lower both sides together to keep enum indices within the same nominal type.
#[register_rule("ReprGeneral", 9400, [Eq, Neq])]
fn enum_equality(expr: &Expression, _: &SymbolTable) -> ApplicationResult {
    let (lhs, rhs, neq) = as_eq_or_neq(expr)?;
    let ty = lhs.return_type();
    if !matches!(ty, ReturnType::EnumeratedType(_)) || ty != rhs.return_type() {
        return Err(RuleNotApplicable);
    }

    fn lower(expr: &Expression) -> Option<Expression> {
        if let Some(Literal::EnumVariant(member)) = eval_constant(expr) {
            return Some(i32::try_from(member.variant).ok()?.into());
        }
        let Expression::Atomic(_, Atom::Reference(reference)) = expr else {
            return None;
        };
        Some(Reference::new(reference.get_repr_as::<EnumAsInt>()?.value.clone()).into())
    }

    let lhs = Moo::new(lower(lhs).ok_or(RuleNotApplicable)?);
    let rhs = Moo::new(lower(rhs).ok_or(RuleNotApplicable)?);
    Ok(RuleEffect::pure(if neq {
        Expression::Neq(Metadata::new(), lhs, rhs)
    } else {
        Expression::Eq(Metadata::new(), lhs, rhs)
    }))
}

#[cfg(test)]
mod tests {
    use conjure_cp::ast::Name;
    use conjure_cp::pipeline::Pipeline;
    use conjure_cp::solutions::SolutionOptions;

    #[test]
    fn solves_set_membership_for_tuples_of_enum_values() {
        let pipeline = Pipeline::new(Default::default()).unwrap();
        let model = pipeline
            .compile(
                "language Essence 1.3
                letting Colour be new type enum {Red, Yellow, Green}
                find x, y : tuple (Colour(Red, Green), Colour)
                such that {x, y} in {{(Red, Yellow), (Green, Red)}}",
                None,
            )
            .unwrap();
        let solutions = pipeline
            .solve(
                model,
                &SolutionOptions {
                    number_of_solutions: 0,
                    ..Default::default()
                },
            )
            .unwrap();
        let x = Name::from("x");
        let y = Name::from("y");
        let mut pairs = solutions
            .iter()
            .map(|solution| (solution[&x].to_string(), solution[&y].to_string()))
            .collect::<Vec<_>>();
        pairs.sort();
        assert_eq!(
            pairs,
            [
                ("(Green,Red)".to_owned(), "(Red,Yellow)".to_owned()),
                ("(Red,Yellow)".to_owned(), "(Green,Red)".to_owned()),
            ]
        );
    }
}
