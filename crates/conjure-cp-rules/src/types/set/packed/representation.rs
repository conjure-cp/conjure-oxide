use crate::shared::representation_prelude::*;
use conjure_cp::ast::{GroundDomain, Moo, Range, Reference, eval_constant};
use conjure_cp::{domain_int, essence_expr, into_matrix_expr, matrix_expr, range};

/// Packed masks use a signed `i32`, leaving 30 usable element bits.
const MAX_INNER_DOMAIN_SIZE: u32 = 30;

register_representation!(
    SetPacked("packed")
    struct State<T> {
        /// The single integer variable, domain, or literal holding the subset bit mask.
        pub packed: T,
        /// Inner-domain values in bit-position order.
        pub elements: Moo<Vec<Literal>>,
        /// Inclusive cardinality bounds.
        pub cardinality: (u32, u32),
        /// Number of bit masks represented by `packed`, including cardinality-invalid masks.
        pub total_size: i32
    }
    impl State<DeclarationPtr> {
        pub fn packed_ref(&self) -> Reference {
            Reference::new(self.packed.clone())
        }

        pub fn packed_expr(&self) -> Expression {
            self.packed_ref().into()
        }

        pub fn cardinality_expr(&self) -> Expression {
            let (min, max) = self.cardinality;
            if min == max {
                return (min as i32).into();
            }
            self.decoded_cardinality_expr()
        }

        fn decoded_cardinality_expr(&self) -> Expression {
            let bits = (0..self.elements.len())
                .map(|index| self.bit_expr(index))
                .collect::<Vec<_>>();
            Expression::Sum(Metadata::new(), Moo::new(into_matrix_expr!(bits)))
        }

        fn bit_expr(&self, index: usize) -> Expression {
            let divisor = 1i32
                .checked_shl(index as u32)
                .expect("validated packed set bit index");
            let packed = self.packed_expr();
            essence_expr!((&packed / &divisor) % 2)
        }

        pub fn membership_expr(&self, member: Expression) -> Expression {
            if let Some(value) = eval_constant(&member) {
                return self
                    .elements
                    .iter()
                    .position(|candidate| candidate.essence_cmp(&value).is_eq())
                    .map(|index| self.element_occurs_expr(index))
                    .unwrap_or_else(|| false.into());
            }

            let choices = self
                .elements
                .iter()
                .enumerate()
                .map(|(index, value)| {
                    let value_matches = Expression::Eq(
                        Metadata::new(),
                        Moo::new(member.clone()),
                        Moo::new(value.clone().into()),
                    );
                    Expression::And(
                        Metadata::new(),
                        Moo::new(matrix_expr![value_matches, self.element_occurs_expr(index)]),
                    )
                })
                .collect();
            Expression::Or(Metadata::new(), Moo::new(into_matrix_expr!(choices)))
        }

        /// Return whether the element at `index` occurs in the packed rank.
        pub fn element_occurs_expr(&self, index: usize) -> Expression {
            Expression::Eq(
                Metadata::new(),
                Moo::new(self.bit_expr(index)),
                Moo::new(1.into()),
            )
        }

        /// Lower `self ⊆ superset` via membership of each packable element.
        pub fn subset_expr(&self, superset: Expression) -> Expression {
            let constraints = self
                .elements
                .iter()
                .cloned()
                .map(|value| {
                    let in_self = self.membership_expr(value.clone().into());
                    let in_super = Expression::In(
                        Metadata::new(),
                        Moo::new(value.into()),
                        Moo::new(superset.clone()),
                    );
                    Expression::Or(
                        Metadata::new(),
                        Moo::new(matrix_expr![
                            Expression::Not(Metadata::new(), Moo::new(in_self)),
                            in_super,
                        ]),
                    )
                })
                .collect::<Vec<_>>();
            Expression::And(Metadata::new(), Moo::new(into_matrix_expr![constraints]))
        }

        /// Lower equality with a set literal to equality on the packed rank.
        pub fn equality_to_literal_expr(&self, elems: &[Literal]) -> Option<Expression> {
            let mut mask = 0u32;
            for elem in elems {
                let index = self
                    .elements
                    .iter()
                    .position(|candidate| candidate.essence_cmp(elem).is_eq())?;
                let bit = 1u32 << index;
                if mask & bit != 0 {
                    return None;
                }
                mask |= bit;
            }
            let cardinality = mask.count_ones();
            let (min, max) = self.cardinality;
            if cardinality < min || cardinality > max {
                return None;
            }
            Some(Expression::Eq(
                Metadata::new(),
                Moo::new(self.packed_expr()),
                Moo::new((mask as i32).into()),
            ))
        }
    }
    fn init(dom: DomainPtr) -> Result<State<DomainPtr>, ReprInitError> {
        let domain_err = |msg: &str| ReprInitError::UnsupportedDomain(
            dom.clone(),
            SetPacked::NAME,
            String::from(msg),
        );
        let Some(GroundDomain::Set(attr, inner_dom)) = dom.as_ground() else {
            return Err(domain_err("expected a ground set domain"));
        };

        let inner_len = inner_dom
            .length()
            .map_err(|e| domain_err(&format!("could not enumerate set domain: {e}")))?;
        let inner_len = u32::try_from(inner_len)
            .map_err(|_| domain_err("set inner domain is too large"))?;
        if inner_len > MAX_INNER_DOMAIN_SIZE {
            return Err(domain_err("set inner domain is too large"));
        }
        let cardinality = cardinality_bounds(&attr.size, inner_len)
            .ok_or_else(|| domain_err("invalid or unsupported set cardinality"))?;
        let elements = Moo::new(
            inner_dom
                .values()
                .map_err(|e| domain_err(&format!("could not enumerate set domain: {e}")))?
                .collect(),
        );
        let total_size = 1i32
            .checked_shl(inner_len)
            .ok_or_else(|| domain_err("packed representation would overflow i32"))?;
        let packed = domain_int!(0..(total_size - 1));

        Ok(State { packed, elements, cardinality, total_size })
    }
    fn structural(state: &State<DeclarationPtr>) -> Vec<Expression> {
        let (min, max) = state.cardinality;
        let cardinality = state.decoded_cardinality_expr();
        let min = min as i32;
        let max = max as i32;
        if min == max {
            vec![essence_expr!(&cardinality = &min)]
        } else {
            vec![essence_expr!(r"(&cardinality >= &min) /\ (&cardinality <= &max)")]
        }
    }
    fn down(state: &State<DomainPtr>, value: Literal) -> Result<State<Literal>, ReprDownError> {
        let Literal::AbstractLiteral(AbstractLiteral::Set(elems)) = value else {
            return Err(ReprDownError::BadValue(value, String::from("expected a set literal")));
        };
        let original = Literal::AbstractLiteral(AbstractLiteral::Set(elems.clone()));

        let mut mask = 0i32;
        for elem in &elems {
            let Some(index) = state
                .elements
                .iter()
                .position(|candidate| candidate.essence_cmp(elem).is_eq())
            else {
                return Err(ReprDownError::BadValue(
                    original,
                    format!("element {elem} is outside the set inner domain"),
                ));
            };
            let bit = 1i32 << index;
            if mask & bit != 0 {
                return Err(ReprDownError::BadValue(
                    original,
                    format!("duplicate set element {elem}"),
                ));
            }
            mask |= bit;
        }
        let cardinality = mask.count_ones();
        let (min, max) = state.cardinality;
        if cardinality < min || cardinality > max {
            return Err(ReprDownError::BadValue(
                original,
                "set cardinality is outside the domain bounds".to_string(),
            ));
        }
        Ok(State {
            packed: Literal::Int(mask),
            elements: state.elements.clone(),
            cardinality: state.cardinality,
            total_size: state.total_size,
        })
    }
    fn up(state: State<Literal>) -> Literal {
        let Literal::Int(mask) = state.packed else {
            bug!("expected an integer literal for packed set value, got {}", state.packed);
        };
        if mask < 0 || mask >= state.total_size {
            bug!("packed set mask {mask} is outside its representation domain");
        }
        let mut elems = state
            .elements
            .iter()
            .enumerate()
            .filter(|(index, _)| mask & (1i32 << index) != 0)
            .map(|(_, elem)| elem.clone())
            .collect::<Vec<_>>();
        elems.sort_by_key(ToString::to_string);
        Literal::AbstractLiteral(AbstractLiteral::Set(elems))
    }
    fn compactness(state: &State<DomainPtr>) -> usize {
        let (min, max) = state.cardinality;
        (min..=max)
            .map(|size| binomial(state.elements.len() as u32, size))
            .fold(0usize, usize::saturating_add)
    }
);

fn binomial(n: u32, k: u32) -> usize {
    if k > n {
        return 0;
    }
    let k = k.min(n - k);
    (1..=k).fold(1usize, |value, i| {
        value.saturating_mul((n - k + i) as usize) / i as usize
    })
}

fn cardinality_bounds(size: &Range<i32>, inner_len: u32) -> Option<(u32, u32)> {
    let (min, max) = match size {
        Range::Unbounded => (0, inner_len),
        Range::Single(n) => ((*n).try_into().ok()?, (*n).try_into().ok()?),
        Range::UnboundedR(min) => ((*min).try_into().ok()?, inner_len),
        Range::UnboundedL(max) => (0, (*max).try_into().ok()?),
        Range::Bounded(min, max) => ((*min).try_into().ok()?, (*max).try_into().ok()?),
    };
    // Clamp oversized attributes (e.g. maxSize 3 of int(1..2)) to the inner domain.
    let max = max.min(inner_len);
    (min <= max).then_some((min, max))
}
