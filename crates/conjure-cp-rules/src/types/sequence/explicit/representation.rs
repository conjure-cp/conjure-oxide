use crate::shared::representation_prelude::*;
use conjure_cp::ast::{Domain, GroundDomain, JectivityAttr, Moo, Range, Reference};
use conjure_cp::{domain_int, essence_expr, matrix_expr, range};

register_representation!(
    SequenceExplicit("explicit")
    struct State<T> {
        /// Inclusive lower and upper bounds for the length marker.
        pub size_bounds: (i32, i32),
        /// Values in position order, padded with `padding` after the active length.
        pub values_matrix: T,
        /// Number of active values in `values_matrix`, omitted for fixed-length sequences.
        pub length: Option<T>,
        /// Canonical value stored in every inactive position.
        pub padding: Literal
    }
    impl State<DeclarationPtr> {
        /// Return the active length, using the marker when the length is variable.
        pub fn length_expr(&self) -> Expression {
            match &self.length {
                Some(length) => Reference::new(length.clone()).into(),
                None => self.size_bounds.0.into(),
            }
        }

        /// Return the value stored at a one-based position.
        pub fn slot_expr(&self, index: i32) -> Expression {
            Expression::SafeIndex(
                Metadata::new(),
                Moo::new(Reference::new(self.values_matrix.clone()).into()),
                vec![index.into()],
            )
        }
    }
    fn init(dom: DomainPtr) -> Result<State<DomainPtr>, ReprInitError> {
        let domain_err = |msg: &str| ReprInitError::UnsupportedDomain(
            dom.clone(),
            SequenceExplicit::NAME,
            String::from(msg),
        );
        let Some(GroundDomain::Sequence(attr, inner_dom)) = dom.as_ground() else {
            return Err(domain_err("expected a ground sequence domain"));
        };
        if attr.jectivity != JectivityAttr::None {
            return Err(domain_err(
                "explicit representation currently only supports non-jective sequences",
            ));
        }
        let size_bounds @ (min, max) = size_bounds(&attr.size)
            .ok_or_else(|| domain_err("explicit representation requires a bounded maximum size"))?;
        if max == 0 {
            return Err(domain_err("explicit representation does not support an always-empty sequence"));
        }
        let padding = inner_dom
            .values()
            .map_err(|e| domain_err(&format!("could not enumerate sequence inner domain: {e}")))?
            .next()
            .ok_or_else(|| domain_err("sequence inner domain is empty"))?;
        let length = (min != max).then(|| domain_int!(min..max));
        let values_matrix = Domain::matrix(inner_dom.clone().into(), vec![domain_int!(1..max)]);
        Ok(State {
            size_bounds,
            values_matrix,
            length,
            padding,
        })
    }
    fn structural(state: &State<DeclarationPtr>) -> Vec<Expression> {
        let (_min, max) = state.size_bounds;
        let length = state.length_expr();
        // Inactive positions (beyond the active length) hold one canonical value, so that
        // reconstruction and equality do not need to reason about "don't care" padding.
        (1..=max)
            .map(|i| {
                let elem = state.slot_expr(i);
                Expression::Or(
                    Metadata::new(),
                    Moo::new(matrix_expr![
                        essence_expr!(&i <= &length),
                        Expression::Eq(
                            Metadata::new(),
                            Moo::new(elem),
                            Moo::new(state.padding.clone().into()),
                        ),
                    ]),
                )
            })
            .collect()
    }
    fn down(state: &State<DomainPtr>, value: Literal) -> Result<State<Literal>, ReprDownError> {
        let Literal::AbstractLiteral(AbstractLiteral::Sequence(mut elems)) = value else {
            return Err(ReprDownError::BadValue(value, String::from("expected a sequence literal")));
        };

        let (min, max) = state.size_bounds;
        let elems_sz = elems.len() as i32;
        if elems_sz < min || elems_sz > max {
            return Err(ReprDownError::BadValue(
                AbstractLiteral::Sequence(elems).into(),
                format!("expected between {min} and {max} elements, got {elems_sz}"),
            ));
        }

        elems.resize(max as usize, state.padding.clone());
        Ok(State {
            size_bounds: (min, max),
            length: (min != max).then(|| Literal::from(elems_sz)),
            values_matrix: Literal::from(into_matrix!(elems)),
            padding: state.padding.clone(),
        })
    }
    fn up(state: State<Literal>) -> Literal {
        let Literal::AbstractLiteral(AbstractLiteral::Matrix(mut elems, _)) = state.values_matrix else {
            bug!("expected sequence values to be a matrix, got {}", state.values_matrix)
        };
        let length = match state.length {
            Some(Literal::Int(length)) => length,
            Some(other) => bug!("expected sequence length to be an integer, got {other}"),
            None => state.size_bounds.0,
        };
        elems.truncate(length as usize);
        Literal::AbstractLiteral(AbstractLiteral::Sequence(elems))
    }
);

/// Inclusive `(min, max)` length bounds for the explicit representation.
///
/// Mirrors Conjure's `hasMaxSize` check: the representation only applies when an explicit
/// maximum is known (`size`, `maxSize` or `minSize, maxSize`), never for an unbounded size.
fn size_bounds(size: &Range<i32>) -> Option<(i32, i32)> {
    let (min, max) = match size {
        Range::Single(n) => (*n, *n),
        Range::UnboundedR(_) | Range::Unbounded => return None,
        Range::UnboundedL(max) => (0, *max),
        Range::Bounded(min, max) => (*min, *max),
    };
    (0 <= min && min <= max).then_some((min, max))
}
