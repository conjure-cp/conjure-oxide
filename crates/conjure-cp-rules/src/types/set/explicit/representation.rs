use crate::shared::representation_prelude::*;
use conjure_cp::ast::{Domain, GroundDomain, Moo, Range, Reference};
use conjure_cp::{domain_int, essence_expr, into_matrix_expr, matrix_expr, range};

register_representation!(
    SetExplicit("explicit")
    struct State<T> {
        /// Inclusive lower and upper bounds for the marker.
        pub cardinality: (i32, i32),
        /// Ordered values, padded after the active marker position.
        pub elems_matrix: T,
        /// Number of active values in `elems_matrix`, omitted for fixed-size sets.
        pub set_size: Option<T>,
        /// Canonical value stored in every inactive position.
        pub padding: Literal
    }
    impl State<DeclarationPtr> {
        /// Return the set cardinality, using the marker when the size is variable.
        pub fn cardinality_expr(&self) -> Expression {
            match &self.set_size {
                Some(set_size) => Reference::new(set_size.clone()).into(),
                None => self.cardinality.0.into(),
            }
        }

        /// Return the value stored at a one-based representation slot.
        pub fn slot_expr(&self, index: i32) -> Expression {
            self.slot_expr_at(index.into())
        }

        /// Return the value stored at a representation-slot expression.
        pub fn slot_expr_at(&self, index: Expression) -> Expression {
            Expression::SafeIndex(
                Metadata::new(),
                Moo::new(Reference::new(self.elems_matrix.clone()).into()),
                vec![index],
            )
        }

        /// Return whether a one-based representation slot is active.
        pub fn slot_is_active_expr(&self, index: i32) -> Expression {
            Expression::Leq(
                Metadata::new(),
                Moo::new(index.into()),
                Moo::new(self.cardinality_expr()),
            )
        }

        /// Lower membership to active-slot equality checks.
        pub fn membership_expr(&self, member: Expression) -> Expression {
            let (_, max) = self.cardinality;
            let choices = (1..=max)
                .map(|index| {
                    let equality = Expression::Eq(
                        Metadata::new(),
                        Moo::new(self.slot_expr(index)),
                        Moo::new(member.clone()),
                    );
                    Expression::And(
                        Metadata::new(),
                        Moo::new(matrix_expr![self.slot_is_active_expr(index), equality]),
                    )
                })
                .collect::<Vec<_>>();
            Expression::Or(Metadata::new(), Moo::new(into_matrix_expr![choices]))
        }

        /// Lower subset inclusion to membership checks for every active slot.
        pub fn subset_expr(&self, superset: Expression) -> Expression {
            let (_, max) = self.cardinality;
            let constraints = (1..=max)
                .map(|index| {
                    let inactive = Expression::Lt(
                        Metadata::new(),
                        Moo::new(self.cardinality_expr()),
                        Moo::new(index.into()),
                    );
                    let membership = Expression::In(
                        Metadata::new(),
                        Moo::new(self.slot_expr(index)),
                        Moo::new(superset.clone()),
                    );
                    Expression::Or(
                        Metadata::new(),
                        Moo::new(matrix_expr![inactive, membership]),
                    )
                })
                .collect::<Vec<_>>();
            Expression::And(Metadata::new(), Moo::new(into_matrix_expr![constraints]))
        }

        /// Lower equality with a set literal through the marker and membership encoding.
        pub fn equality_to_literal_expr(&self, elems: &[Literal]) -> Expression {
            let size_equality = Expression::Eq(
                Metadata::new(),
                Moo::new(self.cardinality_expr()),
                Moo::new((elems.len() as i32).into()),
            );
            let constraints = std::iter::once(size_equality)
                .chain(
                    elems
                        .iter()
                        .cloned()
                        .map(|elem| self.membership_expr(elem.into())),
                )
                .collect::<Vec<_>>();
            Expression::And(Metadata::new(), Moo::new(into_matrix_expr![constraints]))
        }

        /// Lower equality with another marker set through cardinality and active membership.
        pub fn equality_expr(&self, other: &Self) -> Expression {
            let size_equality = Expression::Eq(
                Metadata::new(),
                Moo::new(self.cardinality_expr()),
                Moo::new(other.cardinality_expr()),
            );
            let (_, max) = self.cardinality;
            let constraints = std::iter::once(size_equality)
                .chain((1..=max).map(|index| {
                    let inactive = Expression::Lt(
                        Metadata::new(),
                        Moo::new(self.cardinality_expr()),
                        Moo::new(index.into()),
                    );
                    Expression::Or(
                        Metadata::new(),
                        Moo::new(matrix_expr![
                            inactive,
                            other.membership_expr(self.slot_expr(index)),
                        ]),
                    )
                }))
                .collect::<Vec<_>>();
            Expression::And(Metadata::new(), Moo::new(into_matrix_expr![constraints]))
        }
    }
    fn init(dom: DomainPtr) -> Result<State<DomainPtr>, ReprInitError> {
        let domain_err = |msg: &str| ReprInitError::UnsupportedDomain(
            dom.clone(),
            SetExplicit::NAME,
            String::from(msg),
        );
        let Some(GroundDomain::Set(attr, inner_dom)) = dom.as_ground() else {
            return Err(domain_err("expected a ground set domain"));
        };
        let inner_len = inner_dom
            .length()
            .map_err(|e| domain_err(&format!("could not enumerate set domain: {e}")))?;
        let cardinality @ (min, max) = cardinality_bounds(&attr.size, inner_len)
            .ok_or_else(|| domain_err("invalid or unsupported set cardinality"))?;
        if max == 0 {
            return Err(domain_err("explicit representation does not support an always-empty set"));
        }
        let padding = inner_dom
            .values()
            .map_err(|e| domain_err(&format!("could not enumerate set domain: {e}")))?
            .next()
            .ok_or_else(|| domain_err("set inner domain is empty"))?;
        let set_size = (min != max).then(|| domain_int!(min..max));
        let elems_matrix = Domain::matrix(inner_dom.clone().into(), vec![domain_int!(1..max)]);
        Ok(State {
            elems_matrix,
            set_size,
            cardinality,
            padding,
        })
    }
    fn structural(state: &State<DeclarationPtr>) -> Vec<Expression> {
        let (_, max) = state.cardinality;
        let _size = state.cardinality_expr();
        let mut constraints = (2..=max)
            .map(|_i| {
                let _prev = _i - 1;
                let prev_elem = state.slot_expr(_prev);
                let elem = state.slot_expr(_i);
                let ordering = Expression::LexLt(
                    Metadata::new(),
                    Moo::new(matrix_expr![prev_elem]),
                    Moo::new(matrix_expr![elem]),
                );
                Expression::Or(
                    Metadata::new(),
                    Moo::new(matrix_expr![essence_expr!(&_size < &_i), ordering]),
                )
            })
            .collect::<Vec<_>>();
        // Inactive positions have one canonical value and are ignored by reconstruction.
        constraints.extend((1..=max).map(|i| {
            let elem = state.slot_expr(i);
            Expression::Or(
                Metadata::new(),
                Moo::new(matrix_expr![
                    essence_expr!(&i <= &_size),
                    Expression::Eq(
                        Metadata::new(),
                        Moo::new(elem),
                        Moo::new(state.padding.clone().into()),
                    ),
                ]),
            )
        }));
        constraints
    }
    fn down(state: &State<DomainPtr>, value: Literal) -> Result<State<Literal>, ReprDownError> {
        let Literal::AbstractLiteral(AbstractLiteral::Set(mut elems)) = value else {
            return Err(ReprDownError::BadValue(value, String::from("expected a set literal")));
        };

        let cardinality @ (min, max) = state.cardinality;
        let elems_sz = elems.len() as i32;
        if elems_sz < min || elems_sz > max {
            return Err(ReprDownError::BadValue(
                AbstractLiteral::Set(elems).into(),
                format!("expected between {min} and {max} elements, got {elems_sz}"),
            ));
        }

        elems.sort_by_key(ToString::to_string);
        elems.resize(max as usize, state.padding.clone());
        Ok(State {
            cardinality,
            set_size: (min != max).then(|| Literal::from(elems_sz)),
            elems_matrix: Literal::from(into_matrix!(elems)),
            padding: state.padding.clone(),
        })
    }
    fn up(state: State<Literal>) -> Literal {
        let Literal::AbstractLiteral(AbstractLiteral::Matrix(mut elems, _)) = state.elems_matrix else {
            bug!("expected set elements to be a matrix, got {}", state.elems_matrix)
        };
        let set_size = match state.set_size {
            Some(Literal::Int(set_size)) => set_size,
            Some(other) => bug!("expected set size to be an integer, got {other}"),
            None => state.cardinality.0,
        };
        elems.truncate(set_size as usize);
        Literal::AbstractLiteral(AbstractLiteral::Set(elems))
    }
);

fn cardinality_bounds(size: &Range<i32>, inner_len: u64) -> Option<(i32, i32)> {
    let inner_len = i32::try_from(inner_len).ok()?;
    let (min, max) = match size {
        Range::Unbounded => (0, inner_len),
        Range::Single(n) => (*n, *n),
        Range::UnboundedR(min) => (*min, inner_len),
        Range::UnboundedL(max) => (0, *max),
        Range::Bounded(min, max) => (*min, *max),
    };
    // Clamp oversized attributes (e.g. maxSize 3 of int(1..2)) to the inner domain.
    let max = max.min(inner_len);
    (0 <= min && min <= max).then_some((min, max))
}
