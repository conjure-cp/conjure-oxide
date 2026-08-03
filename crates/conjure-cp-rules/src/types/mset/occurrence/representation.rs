use super::super::mset_bounds;
use crate::shared::representation_prelude::*;
use conjure_cp::ast::{GroundDomain, Moo, Reference};
use conjure_cp::{domain_int, essence_expr, into_matrix_expr, matrix_expr, range};
use std::collections::VecDeque;

const MAX_INNER_DOMAIN_SIZE: usize = 100;

register_representation!(
    MSetOccurrence("occurrence")
    struct State<T> {
        pub cardinality: (i32, i32),
        pub occurrence: (i32, i32),
        pub occurs: Moo<Vec<(Literal, T)>>
    }
    impl State<DeclarationPtr> {
        pub fn cardinality_expr(&self) -> Expression {
            let terms = self.occurs.iter()
                .map(|(_, declaration)| Expression::from(Reference::new(declaration.clone())))
                .collect::<Vec<_>>();
            Expression::Sum(Metadata::new(), Moo::new(into_matrix_expr!(terms)))
        }

        pub fn frequency_expr(&self, member: Expression) -> Expression {
            let terms = self.occurs.iter().map(|(value, declaration)| {
                let matches = Expression::ToInt(
                    Metadata::new(),
                    Moo::new(Expression::Eq(
                        Metadata::new(),
                        Moo::new(member.clone()),
                        Moo::new(value.clone().into()),
                    )),
                );
                Expression::Product(
                    Metadata::new(),
                    Moo::new(matrix_expr![matches, Expression::from(Reference::new(declaration.clone()))]),
                )
            }).collect::<Vec<_>>();
            Expression::Sum(Metadata::new(), Moo::new(into_matrix_expr!(terms)))
        }

        pub fn membership_expr(&self, member: Expression) -> Expression {
            Expression::Gt(Metadata::new(), Moo::new(self.frequency_expr(member)), Moo::new(0.into()))
        }

        pub fn equality_to_literal_expr(&self, elems: &[Literal]) -> Expression {
            let constraints = self.occurs.iter().map(|(value, declaration)| {
                let count = elems.iter().filter(|elem| elem.essence_cmp(value).is_eq()).count() as i32;
                Expression::Eq(
                    Metadata::new(),
                    Moo::new(Expression::from(Reference::new(declaration.clone()))),
                    Moo::new(count.into()),
                )
            }).collect::<Vec<_>>();
            Expression::And(Metadata::new(), Moo::new(into_matrix_expr!(constraints)))
        }

        pub fn equality_expr(&self, other: &Self) -> Expression {
            let constraints = self.occurs.iter().zip(other.occurs.iter()).map(
                |((lhs_value, lhs), (rhs_value, rhs))| {
                    debug_assert!(lhs_value.essence_cmp(rhs_value).is_eq());
                    Expression::Eq(
                        Metadata::new(),
                        Moo::new(Expression::from(Reference::new(lhs.clone()))),
                        Moo::new(Expression::from(Reference::new(rhs.clone()))),
                    )
                }
            ).collect::<Vec<_>>();
            Expression::And(Metadata::new(), Moo::new(into_matrix_expr!(constraints)))
        }

        pub fn symmetry_ordering_expr(&self) -> Expression {
            let entries = self.occurs.iter().map(|(_, declaration)| {
                let count = Expression::from(Reference::new(declaration.clone()));
                essence_expr!(-(&count))
            }).collect::<Vec<_>>();
            into_matrix_expr!(entries)
        }
    }
    fn init(dom: DomainPtr) -> Result<State<DomainPtr>, ReprInitError> {
        let domain_err = |message: &str| ReprInitError::UnsupportedDomain(dom.clone(), MSetOccurrence::NAME, message.to_owned());
        let Some(GroundDomain::MSet(attrs, inner)) = dom.as_ground() else {
            return Err(domain_err("expected a ground multiset domain"));
        };
        let values = inner.values()
            .map_err(|error| domain_err(&format!("could not enumerate multiset domain: {error}")))?
            .collect::<Vec<_>>();
        if values.len() > MAX_INNER_DOMAIN_SIZE {
            return Err(domain_err("multiset inner domain is too large"));
        }
        let bounds = mset_bounds(attrs, values.len())
            .ok_or_else(|| domain_err("multiset attributes do not define a finite domain"))?;
        let (min_occurrence, max_occurrence) = bounds.occurrence;
        let occurs = values.into_iter().map(|value| (value, domain_int!(min_occurrence..max_occurrence))).collect();
        Ok(State { cardinality: bounds.cardinality, occurrence: bounds.occurrence, occurs: Moo::new(occurs) })
    }
    fn structural(state: &State<DeclarationPtr>) -> Vec<Expression> {
        let (min, max) = state.cardinality;
        let cardinality = state.cardinality_expr();
        if min == max {
            vec![essence_expr!(&cardinality = &min)]
        } else {
            vec![essence_expr!(r"(&cardinality >= &min) /\ (&cardinality <= &max)")]
        }
    }
    fn down(state: &State<DomainPtr>, value: Literal) -> Result<State<Literal>, ReprDownError> {
        let Literal::AbstractLiteral(AbstractLiteral::MSet(elems)) = value else {
            return Err(ReprDownError::BadValue(value, "expected a multiset literal".to_owned()));
        };
        let original = Literal::AbstractLiteral(AbstractLiteral::MSet(elems.clone()));
        let size = elems.len() as i32;
        if size < state.cardinality.0 || size > state.cardinality.1 {
            return Err(ReprDownError::BadValue(original, "multiset cardinality is outside its bounds".to_owned()));
        }
        let occurs = state.occurs.iter().map(|(candidate, _)| {
            let count = elems.iter().filter(|elem| elem.essence_cmp(candidate).is_eq()).count() as i32;
            (candidate.clone(), Literal::Int(count))
        }).collect();
        if elems.iter().any(|elem| !state.occurs.iter().any(|(candidate, _)| candidate.essence_cmp(elem).is_eq())) {
            return Err(ReprDownError::BadValue(original, "multiset contains an element outside its domain".to_owned()));
        }
        Ok(State { cardinality: state.cardinality, occurrence: state.occurrence, occurs: Moo::new(occurs) })
    }
    fn up(state: State<Literal>) -> Literal {
        let mut elems = Vec::new();
        for (value, count) in state.occurs.iter() {
            let Literal::Int(count) = count else { bug!("expected an integer occurrence count, got {count}") };
            elems.extend(std::iter::repeat_n(value.clone(), *count as usize));
        }
        Literal::AbstractLiteral(AbstractLiteral::MSet(elems))
    }
    fn repr_vars(state: &State<DeclarationPtr>) -> VecDeque<DeclarationPtr> {
        state.occurs.iter().map(|(_, declaration)| declaration.clone()).collect()
    }
    fn compactness(state: &State<DomainPtr>) -> usize {
        state.occurs.iter()
            .map(|(_, domain)| conjure_cp::representation::default_impls::domain_size(domain))
            .fold(1usize, usize::saturating_mul)
    }
);
