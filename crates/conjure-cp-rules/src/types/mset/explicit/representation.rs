use super::super::mset_bounds;
use crate::shared::representation_prelude::*;
use conjure_cp::ast::{Domain, GroundDomain, Moo, Reference};
use conjure_cp::{domain_int, essence_expr, into_matrix_expr, matrix_expr, range};

register_representation!(
    MSetExplicit("explicit")
    struct State<T> {
        /// Inclusive cardinality bounds.
        pub cardinality: (i32, i32),
        /// Inclusive per-value occurrence bounds.
        pub occurrence: (i32, i32),
        /// Sorted values, padded after the active prefix.
        pub elems_matrix: T,
        /// Active-prefix length, omitted for fixed-size multisets.
        pub mset_size: Option<T>,
        /// Canonical inactive-slot value.
        pub padding: Literal,
        /// Values in the finite inner domain.
        pub elements: Moo<Vec<Literal>>
    }
    impl State<DeclarationPtr> {
        pub fn cardinality_expr(&self) -> Expression {
            self.mset_size
                .as_ref()
                .map(|size| Reference::new(size.clone()).into())
                .unwrap_or_else(|| self.cardinality.0.into())
        }

        pub fn slot_expr_at(&self, index: Expression) -> Expression {
            Expression::SafeIndex(
                Metadata::new(),
                Moo::new(Reference::new(self.elems_matrix.clone()).into()),
                vec![index],
            )
        }

        pub fn slot_expr(&self, index: i32) -> Expression {
            self.slot_expr_at(index.into())
        }

        pub fn slot_is_active_expr(&self, index: i32) -> Expression {
            let cardinality = self.cardinality_expr();
            essence_expr!(&index <= &cardinality)
        }

        pub fn frequency_expr(&self, member: Expression) -> Expression {
            let (_, max) = self.cardinality;
            let terms = (1..=max)
                .map(|index| {
                    let equality = Expression::Eq(
                        Metadata::new(),
                        Moo::new(self.slot_expr(index)),
                        Moo::new(member.clone()),
                    );
                    let active_and_equal = Expression::And(
                        Metadata::new(),
                        Moo::new(matrix_expr![self.slot_is_active_expr(index), equality]),
                    );
                    Expression::ToInt(Metadata::new(), Moo::new(active_and_equal))
                })
                .collect::<Vec<_>>();
            Expression::Sum(Metadata::new(), Moo::new(into_matrix_expr!(terms)))
        }

        pub fn membership_expr(&self, member: Expression) -> Expression {
            Expression::Gt(
                Metadata::new(),
                Moo::new(self.frequency_expr(member)),
                Moo::new(0.into()),
            )
        }

        pub fn equality_to_literal_expr(&self, elems: &[Literal]) -> Expression {
            let cardinality = Expression::Eq(
                Metadata::new(),
                Moo::new(self.cardinality_expr()),
                Moo::new((elems.len() as i32).into()),
            );
            let frequencies = self.elements.iter().map(|value| {
                let count = elems
                    .iter()
                    .filter(|elem| elem.essence_cmp(value).is_eq())
                    .count() as i32;
                Expression::Eq(
                    Metadata::new(),
                    Moo::new(self.frequency_expr(value.clone().into())),
                    Moo::new(count.into()),
                )
            });
            Expression::And(
                Metadata::new(),
                Moo::new(into_matrix_expr!(std::iter::once(cardinality).chain(frequencies).collect::<Vec<_>>())),
            )
        }

        pub fn equality_expr(&self, other: &Self) -> Expression {
            let constraints = self.elements.iter().map(|value| {
                Expression::Eq(
                    Metadata::new(),
                    Moo::new(self.frequency_expr(value.clone().into())),
                    Moo::new(other.frequency_expr(value.clone().into())),
                )
            }).collect::<Vec<_>>();
            Expression::And(Metadata::new(), Moo::new(into_matrix_expr!(constraints)))
        }
    }
    fn init(dom: DomainPtr) -> Result<State<DomainPtr>, ReprInitError> {
        let domain_err = |message: &str| ReprInitError::UnsupportedDomain(
            dom.clone(), MSetExplicit::NAME, message.to_owned());
        let Some(GroundDomain::MSet(attrs, inner)) = dom.as_ground() else {
            return Err(domain_err("expected a ground multiset domain"));
        };
        let elements = inner.values()
            .map_err(|error| domain_err(&format!("could not enumerate multiset domain: {error}")))?
            .collect::<Vec<_>>();
        let bounds = mset_bounds(attrs, elements.len())
            .ok_or_else(|| domain_err("multiset attributes do not define a finite domain"))?;
        let (min, max) = bounds.cardinality;
        if max == 0 || elements.is_empty() {
            return Err(domain_err("explicit representation requires a non-empty slot domain"));
        }
        let padding = elements[0].clone();
        Ok(State {
            cardinality: bounds.cardinality,
            occurrence: bounds.occurrence,
            elems_matrix: Domain::matrix(inner.clone().into(), vec![domain_int!(1..max)]),
            mset_size: (min != max).then(|| domain_int!(min..max)),
            padding,
            elements: Moo::new(elements),
        })
    }
    fn structural(state: &State<DeclarationPtr>) -> Vec<Expression> {
        let (_, max) = state.cardinality;
        let size = state.cardinality_expr();
        let mut constraints = (2..=max).map(|index| {
            let ordering = Expression::LexLeq(
                Metadata::new(),
                Moo::new(matrix_expr![state.slot_expr(index - 1)]),
                Moo::new(matrix_expr![state.slot_expr(index)]),
            );
            Expression::Or(
                Metadata::new(),
                Moo::new(matrix_expr![essence_expr!(&size < &index), ordering]),
            )
        }).collect::<Vec<_>>();
        constraints.extend((1..=max).map(|index| {
            Expression::Or(
                Metadata::new(),
                Moo::new(matrix_expr![
                    essence_expr!(&index <= &size),
                    Expression::Eq(
                        Metadata::new(),
                        Moo::new(state.slot_expr(index)),
                        Moo::new(state.padding.clone().into()),
                    ),
                ]),
            )
        }));
        let (min_occurrence, max_occurrence) = state.occurrence;
        for value in state.elements.iter() {
            let frequency = state.frequency_expr(value.clone().into());
            constraints.push(essence_expr!(r"(&frequency >= &min_occurrence) /\ (&frequency <= &max_occurrence)"));
        }
        constraints
    }
    fn down(state: &State<DomainPtr>, value: Literal) -> Result<State<Literal>, ReprDownError> {
        let Literal::AbstractLiteral(AbstractLiteral::MSet(mut elems)) = value else {
            return Err(ReprDownError::BadValue(value, "expected a multiset literal".to_owned()));
        };
        let original = Literal::AbstractLiteral(AbstractLiteral::MSet(elems.clone()));
        let (min, max) = state.cardinality;
        let size = i32::try_from(elems.len()).map_err(|_| ReprDownError::BadValue(original.clone(), "multiset is too large".to_owned()))?;
        if size < min || size > max {
            return Err(ReprDownError::BadValue(original, "multiset cardinality is outside its bounds".to_owned()));
        }
        let (min_occurrence, max_occurrence) = state.occurrence;
        for candidate in state.elements.iter() {
            let count = elems.iter().filter(|elem| elem.essence_cmp(candidate).is_eq()).count() as i32;
            if count < min_occurrence || count > max_occurrence {
                return Err(ReprDownError::BadValue(original, format!("occurrence count for {candidate} is outside its bounds")));
            }
        }
        elems.sort_by_key(ToString::to_string);
        elems.resize(max as usize, state.padding.clone());
        Ok(State {
            cardinality: state.cardinality,
            occurrence: state.occurrence,
            elems_matrix: Literal::from(into_matrix!(elems)),
            mset_size: (min != max).then_some(Literal::Int(size)),
            padding: state.padding.clone(),
            elements: state.elements.clone(),
        })
    }
    fn up(state: State<Literal>) -> Literal {
        let Literal::AbstractLiteral(AbstractLiteral::Matrix(mut elems, _)) = state.elems_matrix else {
            bug!("expected multiset elements to be a matrix, got {}", state.elems_matrix)
        };
        let size = match state.mset_size {
            Some(Literal::Int(size)) => size,
            Some(other) => bug!("expected multiset size to be an integer, got {other}"),
            None => state.cardinality.0,
        };
        elems.truncate(size as usize);
        Literal::AbstractLiteral(AbstractLiteral::MSet(elems))
    }
);
