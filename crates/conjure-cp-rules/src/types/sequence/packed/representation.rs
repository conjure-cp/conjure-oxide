use crate::shared::representation_prelude::*;
use crate::types::product::{canonical_product_literal, symmetry_values};
use conjure_cp::ast::{GroundDomain, JectivityAttr, Moo, Range, Reference};
use conjure_cp::{domain_int, essence_expr, into_matrix_expr, matrix_expr, range};

register_representation!(
    SequencePacked("packed")
    struct State<T> {
        /// The complete sequence encoded as one mixed-radix integer.
        pub packed: T,
        /// Inner-domain values in Conjure symmetry order, shared by every position digit.
        pub values: Moo<Vec<Literal>>,
        /// Inclusive lower and upper bounds for the length.
        pub size_bounds: (i32, i32),
        /// Whether the most significant digit encodes the active length.
        pub has_length_digit: bool,
        /// Place value for each digit (length digit first, if present, then position 1..maxSize).
        pub places: Vec<i32>,
        /// Number of values for each digit.
        pub radices: Vec<i32>,
        /// Number of sequence values represented by `packed`.
        pub total_size: i32,
        /// Jectivity to enforce structurally.
        pub jectivity: JectivityAttr
    }
    impl State<DeclarationPtr> {
        pub fn packed_expr(&self) -> Expression {
            Reference::new(self.packed.clone()).into()
        }

        /// Return the active length, using the marker digit when the length is variable.
        pub fn length_expr(&self) -> Expression {
            if !self.has_length_digit {
                return self.size_bounds.0.into();
            }
            let digit = self.digit_expr(0);
            let min = self.size_bounds.0;
            if min == 0 { digit } else { essence_expr!(&digit + &min) }
        }

        /// Return the value stored at a one-based position.
        pub fn slot_expr(&self, index: i32) -> Expression {
            let offset = if self.has_length_digit { 1 } else { 0 };
            let digit_index = offset + (index - 1) as usize;
            let digit = self.digit_expr(digit_index);
            if let Some(minimum) = contiguous_int_min(&self.values) {
                return match minimum {
                    0 => digit,
                    minimum => essence_expr!(&digit + &minimum),
                };
            }
            let values = self.values.iter().cloned().map(Expression::from).collect::<Vec<_>>();
            Expression::SafeIndex(
                Metadata::new(),
                Moo::new(into_matrix_expr!(values)),
                vec![essence_expr!(&digit + 1)],
            )
        }

        fn digit_expr(&self, digit_index: usize) -> Expression {
            let packed = self.packed_expr();
            let place = self.places[digit_index];
            let radix = self.radices[digit_index];
            match (place, radix, digit_index) {
                (_, 1, _) => Expression::from(0),
                (1, _, 0) => packed,
                (_, _, 0) => essence_expr!(&packed / &place),
                (1, radix, _) => essence_expr!(&packed % &radix),
                (_, radix, _) => essence_expr!((&packed / &place) % &radix),
            }
        }
    }
    impl<T> State<T> {
        /// Encode a full-length (padded), in-domain sequence value into a packed rank.
        pub fn encode(&self, active_length: i32, elems: &[Literal]) -> Option<i32> {
            let mut digits = Vec::with_capacity(self.places.len());
            if self.has_length_digit {
                digits.push(active_length - self.size_bounds.0);
            }
            for elem in elems {
                let elem = canonical_product_literal(elem.clone());
                let digit = self.values.iter().position(|candidate| candidate.essence_cmp(&elem).is_eq())?;
                digits.push(i32::try_from(digit).ok()?);
            }
            if digits.len() != self.places.len() {
                return None;
            }
            digits.iter().zip(&self.places).try_fold(0i32, |packed, (digit, place)| {
                packed.checked_add(digit.checked_mul(*place)?)
            })
        }
    }
    fn init(dom: DomainPtr) -> Result<State<DomainPtr>, ReprInitError> {
        let domain_err = |msg: &str| ReprInitError::UnsupportedDomain(
            dom.clone(),
            SequencePacked::NAME,
            String::from(msg),
        );
        let Some(GroundDomain::Sequence(attr, inner_dom)) = dom.as_ground() else {
            return Err(domain_err("expected a ground sequence domain"));
        };
        let size_bounds @ (min, max) = size_bounds(&attr.size)
            .ok_or_else(|| domain_err("packed representation requires a bounded maximum size"))?;
        if max == 0 {
            return Err(domain_err("packed representation does not support an always-empty sequence"));
        }
        let values = symmetry_values(inner_dom)
            .ok_or_else(|| domain_err("sequence inner domain is not a supported finite packed domain"))?;
        if values.is_empty() {
            return Err(domain_err("sequence inner domain is empty"));
        }
        let value_radix = i32::try_from(values.len())
            .map_err(|_| domain_err("sequence inner domain is too large"))?;

        let has_length_digit = min != max;
        let mut radices = Vec::with_capacity(max as usize + 1);
        if has_length_digit {
            radices.push(max - min + 1);
        }
        radices.extend(std::iter::repeat_n(value_radix, max as usize));

        let mut places = vec![1i32; radices.len()];
        for index in (0..radices.len().saturating_sub(1)).rev() {
            places[index] = places[index + 1]
                .checked_mul(radices[index + 1])
                .ok_or_else(|| domain_err("packed sequence place value would overflow i32"))?;
        }
        let total_size = radices.iter().try_fold(1i32, |size, radix| {
            size.checked_mul(*radix)
                .ok_or_else(|| domain_err("packed sequence domain would overflow i32"))
        })?;

        Ok(State {
            packed: domain_int!(0..(total_size - 1)),
            values: Moo::new(values),
            size_bounds,
            has_length_digit,
            places,
            radices,
            total_size,
            jectivity: attr.jectivity.clone(),
        })
    }
    fn structural(state: &State<DeclarationPtr>) -> Vec<Expression> {
        let (_min, max) = state.size_bounds;
        let length = state.length_expr();
        let mut constraints = Vec::new();

        let injective = matches!(
            state.jectivity,
            JectivityAttr::Injective | JectivityAttr::Bijective
        );
        let surjective = matches!(
            state.jectivity,
            JectivityAttr::Surjective | JectivityAttr::Bijective
        );

        if injective {
            if !state.has_length_digit {
                // Fixed length: every position is always active, so a plain allDiff over the
                // decoded positions is exact.
                let slots: Vec<Expression> = (1..=max).map(|i| state.slot_expr(i)).collect();
                constraints.push(Expression::AllDiff(
                    Metadata::new(),
                    Moo::new(into_matrix_expr![slots]),
                ));
            } else {
                // Variable length: guard pairwise by position activity, matching the same
                // reasoning as the Explicit representation (value-based exemption would wrongly
                // allow two active positions that happen to decode to the same digit-0 value).
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

        if surjective {
            for value in state.values.iter() {
                let value_expr: Expression = value.clone().into();
                let hits: Vec<Expression> = (1..=max)
                    .map(|i| {
                        let active = Expression::Leq(
                            Metadata::new(),
                            Moo::new(i.into()),
                            Moo::new(length.clone()),
                        );
                        let matches_value = Expression::Eq(
                            Metadata::new(),
                            Moo::new(state.slot_expr(i)),
                            Moo::new(value_expr.clone()),
                        );
                        Expression::And(
                            Metadata::new(),
                            Moo::new(matrix_expr![active, matches_value]),
                        )
                    })
                    .collect();
                constraints.push(Expression::Or(Metadata::new(), Moo::new(into_matrix_expr![hits])));
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
        let Some(padding) = state.values.first().cloned() else {
            return Err(ReprDownError::BadValue(
                AbstractLiteral::Sequence(elems).into(),
                String::from("sequence inner domain is empty"),
            ));
        };
        elems.resize(max as usize, padding);

        let packed = state.encode(elems_sz, &elems).ok_or_else(|| ReprDownError::BadValue(
            AbstractLiteral::Sequence(elems.clone()).into(),
            String::from("sequence value is outside its domain"),
        ))?;

        Ok(State {
            packed: Literal::Int(packed),
            values: state.values.clone(),
            size_bounds: state.size_bounds,
            has_length_digit: state.has_length_digit,
            places: state.places.clone(),
            radices: state.radices.clone(),
            total_size: state.total_size,
            jectivity: state.jectivity.clone(),
        })
    }
    fn up(state: State<Literal>) -> Literal {
        let Literal::Int(packed) = state.packed else {
            bug!("expected a packed sequence integer, got {}", state.packed)
        };
        let offset = if state.has_length_digit { 1 } else { 0 };
        let length = if state.has_length_digit {
            let place = state.places[0];
            let radix = state.radices[0];
            (packed / place % radix) + state.size_bounds.0
        } else {
            state.size_bounds.0
        };
        let elems = (0..state.size_bounds.1 as usize)
            .map(|k| {
                let place = state.places[offset + k];
                let radix = state.radices[offset + k];
                let digit = packed / place % radix;
                state.values[digit as usize].clone()
            })
            .take(length as usize)
            .collect();
        Literal::AbstractLiteral(AbstractLiteral::Sequence(elems))
    }
    fn compactness(state: &State<DomainPtr>) -> usize {
        state.total_size as usize
    }
);

fn contiguous_int_min(values: &[Literal]) -> Option<i32> {
    let Literal::Int(minimum) = values.first()? else {
        return None;
    };
    values
        .iter()
        .enumerate()
        .all(|(offset, value)| *value == Literal::Int(*minimum + offset as i32))
        .then_some(*minimum)
}

/// Inclusive `(min, max)` length bounds for the packed representation.
fn size_bounds(size: &Range<i32>) -> Option<(i32, i32)> {
    let (min, max) = match size {
        Range::Single(n) => (*n, *n),
        Range::UnboundedR(_) | Range::Unbounded => return None,
        Range::UnboundedL(max) => (0, *max),
        Range::Bounded(min, max) => (*min, *max),
    };
    (0 <= min && min <= max).then_some((min, max))
}
