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
        /// Canonical value stored in every inactive position (the inner domain's first value).
        pub padding: Literal,
        /// Every value of the inner domain, used by the surjective structural constraint.
        pub inner_values: Moo<Vec<Literal>>,
        /// A witness matrix indexed by inner-domain value, present only for surjective/bijective
        /// sequences: `witness_matrix[v]` is a position whose entry equals `v`, proving `v` is
        /// covered. Encoding surjectivity this way -- a position variable per inner value, tied
        /// to the values matrix through a variable-indexed `SafeIndex` -- lets the Minion backend
        /// lower it to a native `Element` constraint (see `introduce_element_from_index`),
        /// which propagates more strongly than unrolling "some position among the `max`
        /// candidates holds this value" into an explicit disjunction.
        pub witness_matrix: Option<T>,
        /// Jectivity to enforce structurally.
        pub jectivity: JectivityAttr
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

        /// Return the value stored at a (possibly variable) position expression.
        pub fn slot_expr_at(&self, index: Expression) -> Expression {
            Expression::SafeIndex(
                Metadata::new(),
                Moo::new(Reference::new(self.values_matrix.clone()).into()),
                vec![index],
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
        let size_bounds @ (min, max) = size_bounds(&attr.size)
            .ok_or_else(|| domain_err("explicit representation requires a bounded maximum size"))?;
        if max == 0 {
            return Err(domain_err("explicit representation does not support an always-empty sequence"));
        }
        let inner_values: Vec<Literal> = inner_dom
            .values()
            .map_err(|e| domain_err(&format!("could not enumerate sequence inner domain: {e}")))?
            .collect();
        let padding = inner_values
            .first()
            .cloned()
            .ok_or_else(|| domain_err("sequence inner domain is empty"))?;
        let length = (min != max).then(|| domain_int!(min..max));
        let values_matrix = Domain::matrix(inner_dom.clone().into(), vec![domain_int!(1..max)]);
        let surjective = matches!(
            attr.jectivity,
            JectivityAttr::Surjective | JectivityAttr::Bijective
        );
        let witness_matrix = surjective
            .then(|| Domain::matrix(domain_int!(1..max), vec![inner_dom.clone().into()]));
        Ok(State {
            size_bounds,
            values_matrix,
            length,
            padding,
            inner_values: Moo::new(inner_values),
            witness_matrix,
            jectivity: attr.jectivity.clone(),
        })
    }
    fn structural(state: &State<DeclarationPtr>) -> Vec<Expression> {
        let (_min, max) = state.size_bounds;
        let length = state.length_expr();
        // Inactive positions (beyond the active length) hold one canonical value, so that
        // reconstruction and equality do not need to reason about "don't care" padding.
        let mut constraints: Vec<Expression> = (1..=max)
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
            .collect();

        let injective = matches!(
            state.jectivity,
            JectivityAttr::Injective | JectivityAttr::Bijective
        );

        if injective {
            if state.length.is_none() {
                // Fixed length: every position is always active, so a plain allDiff over the
                // whole matrix is exact (there is no padding to exclude).
                let matrix_ref = Reference::new(state.values_matrix.clone()).into();
                constraints.push(Expression::AllDiff(Metadata::new(), Moo::new(matrix_ref)));
            } else {
                // Variable length: padding duplicates the inner domain's first value, so a
                // value-based allDifferentExcept would wrongly exempt two genuinely active
                // positions that happen to share that value. Guard pairwise by position
                // activity instead, matching Conjure's own non-integer-domain fallback.
                for i in 1..=max {
                    for j in (i + 1)..=max {
                        let i_inactive = Expression::Gt(
                            Metadata::new(),
                            Moo::new(i.into()),
                            Moo::new(length.clone()),
                        );
                        let j_inactive = Expression::Gt(
                            Metadata::new(),
                            Moo::new(j.into()),
                            Moo::new(length.clone()),
                        );
                        let neq = Expression::Neq(
                            Metadata::new(),
                            Moo::new(state.slot_expr(i)),
                            Moo::new(state.slot_expr(j)),
                        );
                        constraints.push(Expression::Or(
                            Metadata::new(),
                            Moo::new(matrix_expr![i_inactive, j_inactive, neq]),
                        ));
                    }
                }
            }
        }

        if let Some(witness_matrix) = &state.witness_matrix {
            // Total inverse lookup: every inner-domain value must be hit by some active position,
            // witnessed by an explicit position variable rather than a per-position disjunction.
            for value in state.inner_values.iter() {
                let value_expr: Expression = value.clone().into();
                let witness_pos = Expression::SafeIndex(
                    Metadata::new(),
                    Moo::new(Reference::new(witness_matrix.clone()).into()),
                    vec![value_expr.clone()],
                );
                constraints.push(Expression::Leq(
                    Metadata::new(),
                    Moo::new(witness_pos.clone()),
                    Moo::new(length.clone()),
                ));
                constraints.push(Expression::Eq(
                    Metadata::new(),
                    Moo::new(state.slot_expr_at(witness_pos)),
                    Moo::new(value_expr),
                ));
            }
        }

        constraints
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

        let witness_matrix = if let Some(witness_matrix) = &state.witness_matrix {
            let mut witnesses = Vec::with_capacity(state.inner_values.len());
            for target in state.inner_values.iter() {
                let witness_pos = elems[..elems_sz as usize]
                    .iter()
                    .position(|v| v == target)
                    .map(|idx| Literal::Int((idx + 1) as i32))
                    .ok_or_else(|| {
                        ReprDownError::BadValue(
                            AbstractLiteral::Sequence(elems.clone()).into(),
                            format!("sequence is not surjective: no position holds {target}"),
                        )
                    })?;
                witnesses.push(witness_pos);
            }
            let index_dom = match witness_matrix.as_ref() {
                conjure_cp::ast::Domain::Ground(gd) => match gd.as_ref() {
                    GroundDomain::Matrix(_, idx) => idx[0].clone(),
                    _ => bug!("expected the witness matrix to be a ground matrix domain"),
                },
                _ => bug!("expected the witness matrix domain to be ground"),
            };
            Some(Literal::from(into_matrix![witnesses; index_dom]))
        } else {
            None
        };

        elems.resize(max as usize, state.padding.clone());
        Ok(State {
            size_bounds: (min, max),
            length: (min != max).then(|| Literal::from(elems_sz)),
            values_matrix: Literal::from(into_matrix!(elems)),
            padding: state.padding.clone(),
            inner_values: state.inner_values.clone(),
            witness_matrix,
            jectivity: state.jectivity.clone(),
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
