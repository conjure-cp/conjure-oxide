use super::super::mset_bounds;
use crate::shared::representation_prelude::*;
use conjure_cp::ast::{Domain, GroundDomain, Moo, Reference};
use conjure_cp::{domain_int, essence_expr, into_matrix_expr, matrix_expr, range};

register_representation!(
    MSetCounts("counts")
    struct State<T> {
        /// Inclusive cardinality bounds.
        pub cardinality: (i32, i32),
        /// Inclusive per-value occurrence bounds.
        pub occurrence: (i32, i32),
        /// Counts for the active distinct values, followed by zeroes.
        pub counts_matrix: T,
        /// Strictly increasing active distinct values, followed by padding.
        pub values_matrix: T,
        /// Number of available value/count slots.
        pub max_distinct: i32,
        /// Canonical inactive-slot value.
        pub padding: Literal,
        /// Values in the finite inner domain.
        pub elements: Moo<Vec<Literal>>
    }
    impl State<DeclarationPtr> {
        pub fn count_expr_at(&self, index: Expression) -> Expression {
            Expression::SafeIndex(
                Metadata::new(),
                Moo::new(Reference::new(self.counts_matrix.clone()).into()),
                vec![index],
            )
        }

        pub fn count_expr(&self, index: i32) -> Expression {
            self.count_expr_at(index.into())
        }

        pub fn value_expr_at(&self, index: Expression) -> Expression {
            Expression::SafeIndex(
                Metadata::new(),
                Moo::new(Reference::new(self.values_matrix.clone()).into()),
                vec![index],
            )
        }

        pub fn value_expr(&self, index: i32) -> Expression {
            self.value_expr_at(index.into())
        }

        pub fn cardinality_expr(&self) -> Expression {
            let counts = (1..=self.max_distinct)
                .map(|index| self.count_expr(index))
                .collect::<Vec<_>>();
            Expression::Sum(Metadata::new(), Moo::new(into_matrix_expr!(counts)))
        }

        pub fn frequency_expr(&self, member: Expression) -> Expression {
            let terms = (1..=self.max_distinct)
                .map(|index| {
                    let matches = Expression::ToInt(
                        Metadata::new(),
                        Moo::new(Expression::Eq(
                            Metadata::new(),
                            Moo::new(self.value_expr(index)),
                            Moo::new(member.clone()),
                        )),
                    );
                    Expression::Product(
                        Metadata::new(),
                        Moo::new(matrix_expr![self.count_expr(index), matches]),
                    )
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
            let constraints = self.elements.iter().map(|value| {
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
                Moo::new(into_matrix_expr!(constraints.collect::<Vec<_>>())),
            )
        }

        pub fn equality_expr(&self, other: &Self) -> Expression {
            let constraints = self
                .elements
                .iter()
                .map(|value| {
                    Expression::Eq(
                        Metadata::new(),
                        Moo::new(self.frequency_expr(value.clone().into())),
                        Moo::new(other.frequency_expr(value.clone().into())),
                    )
                })
                .collect::<Vec<_>>();
            Expression::And(Metadata::new(), Moo::new(into_matrix_expr!(constraints)))
        }

        pub fn symmetry_ordering_expr(&self) -> Expression {
            let entries = self
                .elements
                .iter()
                .map(|value| {
                    let frequency = self.frequency_expr(value.clone().into());
                    essence_expr!(-(&frequency))
                })
                .collect::<Vec<_>>();
            into_matrix_expr!(entries)
        }
    }
    fn init(dom: DomainPtr) -> Result<State<DomainPtr>, ReprInitError> {
        let domain_err = |message: &str| ReprInitError::UnsupportedDomain(
            dom.clone(),
            MSetCounts::NAME,
            message.to_owned(),
        );
        let Some(GroundDomain::MSet(attrs, inner)) = dom.as_ground() else {
            return Err(domain_err("expected a ground multiset domain"));
        };
        let elements = inner
            .values()
            .map_err(|error| domain_err(&format!("could not enumerate multiset domain: {error}")))?
            .collect::<Vec<_>>();
        let bounds = mset_bounds(attrs, elements.len())
            .ok_or_else(|| domain_err("multiset attributes do not define a finite domain"))?;
        let (min, max) = bounds.cardinality;
        if max == 0 || elements.is_empty() {
            return Err(domain_err("counts representation requires a non-empty slot domain"));
        }
        let minimum_active_count = bounds.occurrence.0.max(1);
        let max_distinct = i32::try_from(elements.len())
            .unwrap_or(i32::MAX)
            .min(max / minimum_active_count);
        if max_distinct == 0 {
            return Err(domain_err("counts representation has no possible active slots"));
        }
        let padding = elements[0].clone();
        Ok(State {
            cardinality: (min, max),
            occurrence: bounds.occurrence,
            counts_matrix: Domain::matrix(
                domain_int!(0..bounds.occurrence.1),
                vec![domain_int!(1..max_distinct)],
            ),
            values_matrix: Domain::matrix(inner.clone().into(), vec![domain_int!(1..max_distinct)]),
            max_distinct,
            padding,
            elements: Moo::new(elements),
        })
    }
    fn structural(state: &State<DeclarationPtr>) -> Vec<Expression> {
        let mut constraints = Vec::new();
        let cardinality = state.cardinality_expr();
        let (min, max) = state.cardinality;
        constraints.push(essence_expr!(r"(&cardinality >= &min) /\ (&cardinality <= &max)"));

        let min_occurrence = state.occurrence.0;
        for index in 1..=state.max_distinct {
            let count = state.count_expr(index);
            if min_occurrence > 0 {
                constraints.push(essence_expr!(r"(&count = 0) \/ (&count >= &min_occurrence)"));
            }
            constraints.push(Expression::Or(
                Metadata::new(),
                Moo::new(matrix_expr![
                    Expression::Gt(Metadata::new(), Moo::new(count), Moo::new(0.into())),
                    Expression::Eq(
                        Metadata::new(),
                        Moo::new(state.value_expr(index)),
                        Moo::new(state.padding.clone().into()),
                    ),
                ]),
            ));
        }

        for index in 2..=state.max_distinct {
            let previous_count = state.count_expr(index - 1);
            let count = state.count_expr(index);
            constraints.push(Expression::Or(
                Metadata::new(),
                Moo::new(matrix_expr![
                    Expression::Eq(Metadata::new(), Moo::new(count.clone()), Moo::new(0.into())),
                    Expression::Gt(Metadata::new(), Moo::new(previous_count), Moo::new(0.into())),
                ]),
            ));
            let ordering = Expression::LexLt(
                Metadata::new(),
                Moo::new(matrix_expr![state.value_expr(index - 1)]),
                Moo::new(matrix_expr![state.value_expr(index)]),
            );
            constraints.push(Expression::Or(
                Metadata::new(),
                Moo::new(matrix_expr![
                    Expression::Eq(Metadata::new(), Moo::new(count), Moo::new(0.into())),
                    ordering,
                ]),
            ));
        }
        constraints
    }
    fn down(state: &State<DomainPtr>, value: Literal) -> Result<State<Literal>, ReprDownError> {
        let Literal::AbstractLiteral(AbstractLiteral::MSet(elems)) = value else {
            return Err(ReprDownError::BadValue(value, "expected a multiset literal".to_owned()));
        };
        let original = Literal::AbstractLiteral(AbstractLiteral::MSet(elems.clone()));
        let size = i32::try_from(elems.len())
            .map_err(|_| ReprDownError::BadValue(original.clone(), "multiset is too large".to_owned()))?;
        if size < state.cardinality.0 || size > state.cardinality.1 {
            return Err(ReprDownError::BadValue(original, "multiset cardinality is outside its bounds".to_owned()));
        }
        if elems.iter().any(|elem| {
            !state
                .elements
                .iter()
                .any(|candidate| candidate.essence_cmp(elem).is_eq())
        }) {
            return Err(ReprDownError::BadValue(original, "multiset contains an element outside its domain".to_owned()));
        }

        let mut histogram = Vec::<(Literal, i32)>::new();
        for elem in elems {
            if let Some((_, count)) = histogram
                .iter_mut()
                .find(|(candidate, _)| candidate.essence_cmp(&elem).is_eq())
            {
                *count += 1;
            } else {
                histogram.push((elem, 1));
            }
        }
        histogram.sort_by(|(lhs, _), (rhs, _)| lhs.essence_cmp(rhs));
        if histogram.len() > state.max_distinct as usize {
            return Err(ReprDownError::BadValue(original, "multiset has too many distinct values".to_owned()));
        }
        if histogram.iter().any(|(_, count)| {
            *count < state.occurrence.0.max(1) || *count > state.occurrence.1
        }) {
            return Err(ReprDownError::BadValue(original, "multiset occurrence counts are outside their bounds".to_owned()));
        }

        let mut values = histogram
            .iter()
            .map(|(value, _)| value.clone())
            .collect::<Vec<_>>();
        let mut counts = histogram
            .iter()
            .map(|(_, count)| Literal::Int(*count))
            .collect::<Vec<_>>();
        values.resize(state.max_distinct as usize, state.padding.clone());
        counts.resize(state.max_distinct as usize, Literal::Int(0));
        Ok(State {
            cardinality: state.cardinality,
            occurrence: state.occurrence,
            counts_matrix: Literal::from(into_matrix!(counts)),
            values_matrix: Literal::from(into_matrix!(values)),
            max_distinct: state.max_distinct,
            padding: state.padding.clone(),
            elements: state.elements.clone(),
        })
    }
    fn up(state: State<Literal>) -> Literal {
        let Literal::AbstractLiteral(AbstractLiteral::Matrix(counts, _)) = state.counts_matrix else {
            bug!("expected multiset counts to be a matrix, got {}", state.counts_matrix)
        };
        let Literal::AbstractLiteral(AbstractLiteral::Matrix(values, _)) = state.values_matrix else {
            bug!("expected multiset values to be a matrix, got {}", state.values_matrix)
        };
        let mut elems = Vec::new();
        for (value, count) in values.into_iter().zip(counts) {
            let Literal::Int(count) = count else {
                bug!("expected an integer multiset count, got {count}")
            };
            elems.extend(std::iter::repeat_n(value, count as usize));
        }
        Literal::AbstractLiteral(AbstractLiteral::MSet(elems))
    }
);
