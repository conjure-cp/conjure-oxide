use crate::shared::representation_prelude::*;
use crate::types::product::{canonical_product_literal, symmetry_values};
use conjure_cp::ast::{GroundDomain, JectivityAttr, Moo, PartialityAttr, Range, Reference};
use conjure_cp::{domain_int, essence_expr, into_matrix_expr, matrix_expr, range};

register_representation!(
    FunctionPacked("packed")
    struct State<T> {
        /// The whole function encoded as one mixed-radix integer, one digit per domain value in
        /// `domain_values` order.
        pub packed: T,
        /// Every domain value, in the order the digits follow.
        pub domain_values: Moo<Vec<Literal>>,
        /// Codomain values in Conjure symmetry order, shared by every digit.
        pub values: Moo<Vec<Literal>>,
        /// Whether a digit can say "undefined", i.e. whether the function is partial.
        ///
        /// A partial function folds definedness into the value digit rather than carrying a
        /// separate flag digit: digit 0 means the function defines nothing at that domain value,
        /// and digit `d > 0` means it takes `values[d - 1]` there. Keeping the two in one digit
        /// makes the encoding both smaller (radix `m + 1` rather than `2 * m`) and free of
        /// redundant states, since an undefined position has no value left to choose.
        pub partial: bool,
        /// Place value for each digit, one per domain value.
        pub places: Vec<i32>,
        /// Number of values each digit can take: the codomain size, plus one for "undefined".
        pub radix: i32,
        /// Number of function values represented by `packed`.
        pub total_size: i32,
        /// Number of entries a partial function may define; irrelevant for total functions.
        pub size: Range<i32>,
        /// Jectivity to enforce structurally.
        pub jectivity: JectivityAttr
    }
    impl State<DeclarationPtr> {
        pub fn packed_expr(&self) -> Expression {
            Reference::new(self.packed.clone()).into()
        }

        /// The codomain value at a one-based `domain_values` position.
        ///
        /// Meaningless where the function defines nothing, so every use is guarded by
        /// [`State::defined_expr`] at the same position.
        pub fn value_expr(&self, index: i32) -> Expression {
            let digit = self.digit_expr(index);
            let offset = i32::from(self.partial);
            if let Some(minimum) = contiguous_int_min(&self.values) {
                let shift = minimum - offset;
                return match shift {
                    0 => digit,
                    shift => essence_expr!(&digit + &shift),
                };
            }
            let values = self
                .values
                .iter()
                .cloned()
                .map(Expression::from)
                .collect::<Vec<_>>();
            let position = essence_expr!(&digit + 1 - &offset);
            Expression::SafeIndex(
                Metadata::new(),
                Moo::new(into_matrix_expr!(values)),
                vec![position],
            )
        }

        /// Whether the function defines a value at a one-based `domain_values` position.
        pub fn defined_expr(&self, index: i32) -> Expression {
            if !self.partial {
                return true.into();
            }
            let digit = self.digit_expr(index);
            essence_expr!(&digit != 0)
        }

        /// The number of entries the function defines, as a sum over every domain value.
        pub fn defined_count_expr(&self) -> Expression {
            let counts: Vec<Expression> = (1..=self.domain_values.len() as i32)
                .map(|index| {
                    let defined = self.defined_expr(index);
                    essence_expr!(toInt(&defined))
                })
                .collect();
            Expression::Sum(Metadata::new(), Moo::new(into_matrix_expr!(counts)))
        }

        fn digit_expr(&self, index: i32) -> Expression {
            let packed = self.packed_expr();
            let place = self.places[(index - 1) as usize];
            let radix = self.radix;
            match (place, radix, index) {
                (_, 1, _) => Expression::from(0),
                (1, _, 1) => packed,
                (_, _, 1) => essence_expr!(&packed / &place),
                (1, radix, _) => essence_expr!(&packed % &radix),
                (_, radix, _) => essence_expr!((&packed / &place) % &radix),
            }
        }
    }
    impl<T> State<T> {
        /// Encode a function value into a packed rank.
        pub fn encode(&self, pairs: &[(Literal, Literal)]) -> Option<i32> {
            let offset = i32::from(self.partial);
            let mut digits = Vec::with_capacity(self.places.len());
            for key in self.domain_values.iter() {
                let Some((_, value)) = pairs
                    .iter()
                    .find(|(candidate, _)| candidate.essence_cmp(key).is_eq())
                else {
                    if !self.partial {
                        return None;
                    }
                    digits.push(0);
                    continue;
                };
                let value = canonical_product_literal(value.clone());
                let digit = self
                    .values
                    .iter()
                    .position(|candidate| candidate.essence_cmp(&value).is_eq())?;
                digits.push(i32::try_from(digit).ok()? + offset);
            }
            digits
                .iter()
                .zip(&self.places)
                .try_fold(0i32, |packed, (digit, place)| {
                    packed.checked_add(digit.checked_mul(*place)?)
                })
        }
    }
    fn init(dom: DomainPtr) -> Result<State<DomainPtr>, ReprInitError> {
        let domain_err = |msg: &str| ReprInitError::UnsupportedDomain(
            dom.clone(),
            FunctionPacked::NAME,
            String::from(msg),
        );
        let Some(GroundDomain::Function(attr, domain, codomain)) = dom.as_ground() else {
            return Err(domain_err("expected a ground function domain"));
        };

        let domain_values: Vec<Literal> = domain.values()
            .map_err(|e| domain_err(&format!("could not enumerate function domain: {e}")))?
            .collect();
        if domain_values.is_empty() {
            return Err(domain_err("function domain is empty"));
        }
        let values = symmetry_values(codomain)
            .ok_or_else(|| domain_err("function codomain is not a supported finite packed domain"))?;
        if values.is_empty() {
            return Err(domain_err("function codomain is empty"));
        }

        let partial = !matches!(attr.partiality, PartialityAttr::Total);
        let radix = i32::try_from(values.len())
            .ok()
            .and_then(|radix| radix.checked_add(i32::from(partial)))
            .ok_or_else(|| domain_err("function codomain is too large"))?;

        let mut places = vec![1i32; domain_values.len()];
        for index in (0..places.len().saturating_sub(1)).rev() {
            places[index] = places[index + 1]
                .checked_mul(radix)
                .ok_or_else(|| domain_err("packed function place value would overflow i32"))?;
        }
        let total_size = places
            .first()
            .copied()
            .unwrap_or(1)
            .checked_mul(radix)
            .ok_or_else(|| domain_err("packed function domain would overflow i32"))?;

        Ok(State {
            packed: domain_int!(0..(total_size - 1)),
            domain_values: Moo::new(domain_values),
            values: Moo::new(values),
            partial,
            places,
            radix,
            total_size,
            size: attr.size.clone(),
            jectivity: attr.jectivity.clone(),
        })
    }
    fn structural(state: &State<DeclarationPtr>) -> Vec<Expression> {
        let n = state.domain_values.len() as i32;
        let mut constraints = Vec::new();

        // A partial function's size attribute bounds how many entries it defines; a total
        // function's is already pinned by every digit being a real value.
        if state.partial {
            let defined = state.defined_count_expr();
            match &state.size {
                Range::Single(size) => constraints.push(essence_expr!(&defined = &size)),
                Range::Bounded(min, max) => {
                    constraints.push(essence_expr!(&defined >= &min));
                    constraints.push(essence_expr!(&defined <= &max));
                }
                Range::UnboundedR(min) => constraints.push(essence_expr!(&defined >= &min)),
                Range::UnboundedL(max) => constraints.push(essence_expr!(&defined <= &max)),
                Range::Unbounded => {}
            }
        }

        let injective = matches!(
            state.jectivity,
            JectivityAttr::Injective | JectivityAttr::Bijective
        );
        let surjective = matches!(
            state.jectivity,
            JectivityAttr::Surjective | JectivityAttr::Bijective
        );

        if injective {
            // Two positions may share a value only where at least one of them defines nothing,
            // so guard pairwise by definedness rather than reaching for a plain allDiff: an
            // undefined digit is not a value that has to differ from anything.
            for i in 1..=n {
                for j in (i + 1)..=n {
                    let i_value = state.value_expr(i);
                    let j_value = state.value_expr(j);
                    let distinct = essence_expr!(&i_value != &j_value);
                    if !state.partial {
                        constraints.push(distinct);
                        continue;
                    }
                    let both_defined = Expression::And(
                        Metadata::new(),
                        Moo::new(matrix_expr![state.defined_expr(i), state.defined_expr(j)]),
                    );
                    constraints.push(Expression::Imply(
                        Metadata::new(),
                        Moo::new(both_defined),
                        Moo::new(distinct),
                    ));
                }
            }
        }

        if surjective {
            for value in state.values.iter() {
                let value_expr: Expression = value.clone().into();
                let covered: Vec<Expression> = (1..=n)
                    .map(|index| {
                        let position_value = state.value_expr(index);
                        let takes = essence_expr!(&position_value = &value_expr);
                        if !state.partial {
                            return takes;
                        }
                        Expression::And(
                            Metadata::new(),
                            Moo::new(matrix_expr![state.defined_expr(index), takes]),
                        )
                    })
                    .collect();
                constraints.push(Expression::Or(
                    Metadata::new(),
                    Moo::new(into_matrix_expr!(covered)),
                ));
            }
        }

        constraints
    }
    fn down(state: &State<DomainPtr>, value: Literal) -> Result<State<Literal>, ReprDownError> {
        let Literal::AbstractLiteral(AbstractLiteral::Function(pairs)) = value else {
            return Err(ReprDownError::BadValue(value, String::from("expected a function literal")));
        };

        let packed = state.encode(&pairs).ok_or_else(|| ReprDownError::BadValue(
            AbstractLiteral::Function(pairs.clone()).into(),
            String::from("function value is outside its domain"),
        ))?;

        Ok(State {
            packed: Literal::Int(packed),
            domain_values: state.domain_values.clone(),
            values: state.values.clone(),
            partial: state.partial,
            places: state.places.clone(),
            radix: state.radix,
            total_size: state.total_size,
            size: state.size.clone(),
            jectivity: state.jectivity.clone(),
        })
    }
    fn up(state: State<Literal>) -> Literal {
        let Literal::Int(packed) = state.packed else {
            bug!("expected a packed function integer, got {}", state.packed)
        };
        let offset = i32::from(state.partial);
        let pairs = state
            .domain_values
            .iter()
            .zip(&state.places)
            .filter_map(|(key, place)| {
                let digit = packed / place % state.radix;
                if state.partial && digit == 0 {
                    return None;
                }
                Some((
                    key.clone(),
                    state.values[(digit - offset) as usize].clone(),
                ))
            })
            .collect();
        Literal::AbstractLiteral(AbstractLiteral::Function(pairs))
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
