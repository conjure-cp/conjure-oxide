use super::super::mset_bounds;
use crate::shared::representation_prelude::*;
use conjure_cp::ast::{GroundDomain, Moo, Reference, eval_constant};
use conjure_cp::{domain_int, essence_expr, into_matrix_expr, matrix_expr, range};

register_representation!(
    MSetPacked("packed")
    struct State<T> {
        pub packed: T,
        pub elements: Moo<Vec<Literal>>,
        pub cardinality: (i32, i32),
        pub occurrence: (i32, i32),
        pub radix: i32,
        pub total_size: i32
    }
    impl State<DeclarationPtr> {
        pub fn packed_expr(&self) -> Expression {
            Reference::new(self.packed.clone()).into()
        }

        fn digit_expr(&self, index: usize) -> Expression {
            let divisor = self.radix.checked_pow(index as u32).expect("validated packed multiset radix");
            let packed = self.packed_expr();
            let radix = self.radix;
            essence_expr!((&packed / &divisor) % &radix)
        }

        pub fn element_frequency_expr(&self, index: usize) -> Expression {
            let digit = self.digit_expr(index);
            let minimum = self.occurrence.0;
            essence_expr!(&digit + &minimum)
        }

        pub fn frequency_expr(&self, member: Expression) -> Expression {
            if let Some(value) = eval_constant(&member) {
                return self.elements.iter()
                    .position(|candidate| candidate.essence_cmp(&value).is_eq())
                    .map(|index| self.element_frequency_expr(index))
                    .unwrap_or_else(|| 0.into());
            }
            let terms = self.elements.iter().enumerate().map(|(index, value)| {
                let matches = Expression::ToInt(Metadata::new(), Moo::new(Expression::Eq(
                    Metadata::new(), Moo::new(member.clone()), Moo::new(value.clone().into()))));
                Expression::Product(Metadata::new(), Moo::new(matrix_expr![matches, self.element_frequency_expr(index)]))
            }).collect::<Vec<_>>();
            Expression::Sum(Metadata::new(), Moo::new(into_matrix_expr!(terms)))
        }

        pub fn cardinality_expr(&self) -> Expression {
            let terms = (0..self.elements.len()).map(|index| self.element_frequency_expr(index)).collect::<Vec<_>>();
            Expression::Sum(Metadata::new(), Moo::new(into_matrix_expr!(terms)))
        }

        pub fn membership_expr(&self, member: Expression) -> Expression {
            Expression::Gt(Metadata::new(), Moo::new(self.frequency_expr(member)), Moo::new(0.into()))
        }

        pub fn equality_to_literal_expr(&self, elems: &[Literal]) -> Option<Expression> {
            let packed = encode(elems, &self.elements, self.occurrence, self.radix)?;
            let size = elems.len() as i32;
            if size < self.cardinality.0 || size > self.cardinality.1 { return None; }
            Some(Expression::Eq(Metadata::new(), Moo::new(self.packed_expr()), Moo::new(packed.into())))
        }
    }
    fn init(dom: DomainPtr) -> Result<State<DomainPtr>, ReprInitError> {
        let domain_err = |message: &str| ReprInitError::UnsupportedDomain(dom.clone(), MSetPacked::NAME, message.to_owned());
        let Some(GroundDomain::MSet(attrs, inner)) = dom.as_ground() else {
            return Err(domain_err("expected a ground multiset domain"));
        };
        let elements = inner.values()
            .map_err(|error| domain_err(&format!("could not enumerate multiset domain: {error}")))?
            .collect::<Vec<_>>();
        let bounds = mset_bounds(attrs, elements.len())
            .ok_or_else(|| domain_err("multiset attributes do not define a finite domain"))?;
        let radix = bounds.occurrence.1.checked_sub(bounds.occurrence.0).and_then(|value| value.checked_add(1))
            .ok_or_else(|| domain_err("invalid occurrence bounds"))?;
        let total_size = radix.checked_pow(elements.len() as u32)
            .ok_or_else(|| domain_err("packed multiset domain would overflow i32"))?;
        Ok(State {
            packed: domain_int!(0..(total_size - 1)),
            elements: Moo::new(elements),
            cardinality: bounds.cardinality,
            occurrence: bounds.occurrence,
            radix,
            total_size,
        })
    }
    fn structural(state: &State<DeclarationPtr>) -> Vec<Expression> {
        let cardinality = state.cardinality_expr();
        let (min, max) = state.cardinality;
        if min == max { vec![essence_expr!(&cardinality = &min)] }
        else { vec![essence_expr!(r"(&cardinality >= &min) /\ (&cardinality <= &max)")] }
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
        let packed = encode(&elems, &state.elements, state.occurrence, state.radix)
            .ok_or_else(|| ReprDownError::BadValue(original, "multiset occurrence counts are outside their bounds".to_owned()))?;
        Ok(State { packed: Literal::Int(packed), elements: state.elements.clone(), cardinality: state.cardinality, occurrence: state.occurrence, radix: state.radix, total_size: state.total_size })
    }
    fn up(state: State<Literal>) -> Literal {
        let Literal::Int(mut packed) = state.packed else { bug!("expected a packed multiset integer, got {}", state.packed) };
        let mut elems = Vec::new();
        for value in state.elements.iter() {
            let count = packed % state.radix + state.occurrence.0;
            packed /= state.radix;
            elems.extend(std::iter::repeat_n(value.clone(), count as usize));
        }
        Literal::AbstractLiteral(AbstractLiteral::MSet(elems))
    }
    fn compactness(state: &State<DomainPtr>) -> usize {
        valid_count(state.elements.len(), state.cardinality, state.occurrence)
    }
);

fn encode(
    elems: &[Literal],
    elements: &[Literal],
    occurrence: (i32, i32),
    radix: i32,
) -> Option<i32> {
    let mut packed = 0i32;
    let mut place = 1i32;
    for candidate in elements {
        let count = elems
            .iter()
            .filter(|elem| elem.essence_cmp(candidate).is_eq())
            .count() as i32;
        if count < occurrence.0 || count > occurrence.1 {
            return None;
        }
        packed = packed.checked_add((count - occurrence.0).checked_mul(place)?)?;
        place = place.checked_mul(radix)?;
    }
    elems
        .iter()
        .all(|elem| {
            elements
                .iter()
                .any(|candidate| candidate.essence_cmp(elem).is_eq())
        })
        .then_some(packed)
}

fn valid_count(elements: usize, cardinality: (i32, i32), occurrence: (i32, i32)) -> usize {
    let max_sum = cardinality.1.max(0) as usize;
    let mut counts = vec![0usize; max_sum + 1];
    counts[0] = 1;
    for _ in 0..elements {
        let mut next = vec![0usize; max_sum + 1];
        for (sum, ways) in counts.iter().copied().enumerate() {
            for count in occurrence.0..=occurrence.1 {
                let new_sum = sum + count as usize;
                if new_sum <= max_sum {
                    next[new_sum] = next[new_sum].saturating_add(ways);
                }
            }
        }
        counts = next;
    }
    (cardinality.0.max(0) as usize..=max_sum)
        .map(|sum| counts[sum])
        .fold(0usize, usize::saturating_add)
}
