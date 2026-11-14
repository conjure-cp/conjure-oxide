//! Represent unnamed members by integers while retaining their nominal type.
use crate::shared::representation_prelude::*;
use crate::shared::utils::as_eq_or_neq;
use conjure_cp::ast::unnamed::{UnnamedType, UnnamedValue};
use conjure_cp::ast::{Domain, GroundDomain, Moo, Range, Reference, ReturnType, Typeable};
use conjure_cp::rule_engine::{RuleEffect, register_rule};

register_representation!(
    UnnamedAsInt("integer")
    struct State<T> {
        pub ty: Moo<UnnamedType>,
        pub value: T
    }
    fn init(dom: DomainPtr) -> Result<State<DomainPtr>, ReprInitError> {
        let ground = dom.resolve().map_err(|_| ReprInitError::UnsupportedDomain(dom.clone(), UnnamedAsInt::NAME, "expected a ground unnamed domain".into()))?;
        let GroundDomain::UnnamedType(ty) = &*ground else {
            return Err(ReprInitError::UnsupportedDomain(dom, UnnamedAsInt::NAME, "expected an unnamed domain".into()));
        };
        let Ok(size) = i32::try_from(ty.size) else {
            return Err(ReprInitError::UnsupportedDomain(dom, UnnamedAsInt::NAME, "unnamed type is too large".into()));
        };
        Ok(State { ty: ty.clone(), value: Domain::int(vec![Range::Bounded(1, size)]) })
    }
    fn structural(_state: &State<DeclarationPtr>) -> Vec<Expression> { vec![] }
    fn down(state: &State<DomainPtr>, value: Literal) -> Result<State<Literal>, ReprDownError> {
        if let Literal::Unnamed(member) = &value
            && member.ty == state.ty && (1..=state.ty.size).contains(&member.value)
        {
            return Ok(State { ty: state.ty.clone(), value: Literal::Int(member.value as i32) });
        }
        Err(ReprDownError::BadValue(value, "expected a member of the unnamed type".into()))
    }
    fn up(state: State<Literal>) -> Literal {
        let Literal::Int(value) = state.value else { bug!("expected an integer unnamed member") };
        Literal::Unnamed(UnnamedValue { ty: state.ty, value: value as u32 })
    }
);

/// Lower both sides together so integer encodings cannot mix with unnamed members.
#[register_rule("ReprGeneral", 9400, [Eq, Neq])]
fn unnamed_equality(expr: &Expression, _: &SymbolTable) -> ApplicationResult {
    let (lhs, rhs, neq) = as_eq_or_neq(expr)?;
    if !matches!(lhs.return_type(), ReturnType::UnnamedType(_))
        || lhs.return_type() != rhs.return_type()
    {
        return Err(RuleNotApplicable);
    }
    fn lower(expr: &Expression) -> Option<Expression> {
        match expr {
            Expression::Atomic(_, Atom::Reference(reference)) => {
                Some(Reference::new(reference.get_repr_as::<UnnamedAsInt>()?.value.clone()).into())
            }
            Expression::Atomic(_, Atom::Literal(Literal::Unnamed(value))) => {
                Some((value.value as i32).into())
            }
            _ => None,
        }
    }
    let lhs = lower(lhs).ok_or(RuleNotApplicable)?;
    let rhs = lower(rhs).ok_or(RuleNotApplicable)?;
    Ok(RuleEffect::pure(if neq {
        Expression::Neq(Metadata::new(), Moo::new(lhs), Moo::new(rhs))
    } else {
        Expression::Eq(Metadata::new(), Moo::new(lhs), Moo::new(rhs))
    }))
}
