use super::prelude::*;
use conjure_cp::ast::{GroundDomain, Moo, Range, Reference};
use conjure_cp::{domain_int, range};

/// Packed masks use a signed `i32`, leaving 30 usable element bits.
const MAX_INNER_DOMAIN_SIZE: u32 = 30;

register_representation!(
    SetPacked
    struct State<T> {
        /// The single integer variable / domain / literal holding the subset rank.
        pub packed: T,
        /// Inner-domain values in bit-position order.
        pub elements: Moo<Vec<Literal>>,
        /// Inclusive cardinality bounds.
        pub cardinality: (u32, u32),
        /// Number of valid sets represented by `packed`.
        pub total_size: i32
    }
    impl State<DeclarationPtr> {
        pub fn packed_ref(&self) -> Reference {
            Reference::new(self.packed.clone())
        }

        pub fn packed_expr(&self) -> Expression {
            self.packed_ref().into()
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
        let cardinality @ (min, max) = cardinality_bounds(&attr.size, inner_len)
            .ok_or_else(|| domain_err("invalid or unsupported set cardinality"))?;
        let elements = Moo::new(
            inner_dom
                .values()
                .map_err(|e| domain_err(&format!("could not enumerate set domain: {e}")))?
                .collect(),
        );
        let total_size = (min..=max).try_fold(0u64, |total, size| {
            total.checked_add(binomial(inner_len, size)?)
        })
            .and_then(|size| i32::try_from(size).ok())
            .ok_or_else(|| domain_err("packed representation would overflow i32"))?;
        let packed = domain_int!(0..(total_size - 1));

        Ok(State { packed, elements, cardinality, total_size })
    }
    fn structural(_: &State<DeclarationPtr>) -> Vec<Expression> {
        vec![]
    }
    fn down(state: &State<DomainPtr>, value: Literal) -> Result<State<Literal>, ReprDownError> {
        let Literal::AbstractLiteral(AbstractLiteral::Set(elems)) = value else {
            return Err(ReprDownError::BadValue(value, String::from("expected a set literal")));
        };
        let original = Literal::AbstractLiteral(AbstractLiteral::Set(elems.clone()));

        let mut mask = 0i32;
        for elem in &elems {
            let Some(index) = state.elements.iter().position(|candidate| candidate == elem) else {
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
        let rank = rank_mask(mask as u32, state.elements.len() as u32, min);

        Ok(State {
            packed: Literal::Int(rank),
            elements: state.elements.clone(),
            cardinality: state.cardinality,
            total_size: state.total_size,
        })
    }
    fn up(state: State<Literal>) -> Literal {
        let Literal::Int(rank) = state.packed else {
            bug!("expected an integer literal for packed set value, got {}", state.packed);
        };
        if rank < 0 || rank >= state.total_size {
            bug!("packed set rank {rank} is outside its representation domain");
        }
        let mask = unrank_mask(
            rank,
            state.elements.len() as u32,
            state.cardinality,
        );
        let mut elems = state
            .elements
            .iter()
            .enumerate()
            .filter(|(index, _)| mask & (1u32 << index) != 0)
            .map(|(_, elem)| elem.clone())
            .collect::<Vec<_>>();
        elems.sort_by_key(ToString::to_string);
        Literal::AbstractLiteral(AbstractLiteral::Set(elems))
    }
);

fn binomial(n: u32, k: u32) -> Option<u64> {
    if k > n {
        return Some(0);
    }
    let k = k.min(n - k);
    (1..=k).try_fold(1u64, |value, i| {
        value
            .checked_mul(u64::from(n - k + i))?
            .checked_div(u64::from(i))
    })
}

/// Rank a mask first by cardinality, then by colexicographic combination order.
fn rank_mask(mask: u32, inner_len: u32, min_cardinality: u32) -> i32 {
    let cardinality = mask.count_ones();
    let cardinality_offset = (min_cardinality..cardinality)
        .map(|size| binomial(inner_len, size).expect("validated packed set size"))
        .sum::<u64>();
    let mut seen = 0u32;
    let colex_rank = (0..inner_len)
        .filter(|index| mask & (1u32 << index) != 0)
        .map(|index| {
            seen += 1;
            binomial(index, seen).expect("set bit has a valid combination rank")
        })
        .sum::<u64>();
    i32::try_from(cardinality_offset + colex_rank).expect("validated packed set rank")
}

fn unrank_mask(rank: i32, inner_len: u32, (min, max): (u32, u32)) -> u32 {
    let mut rank = rank as u64;
    let cardinality = (min..=max)
        .find(|size| {
            let block_size = binomial(inner_len, *size).expect("validated packed set size");
            if rank < block_size {
                true
            } else {
                rank -= block_size;
                false
            }
        })
        .expect("validated packed set rank has a cardinality block");

    let mut mask = 0u32;
    let mut upper = inner_len;
    for selected in (1..=cardinality).rev() {
        let index = (selected - 1..upper)
            .rev()
            .find(|index| binomial(*index, selected).is_some_and(|value| value <= rank))
            .expect("valid colexicographic rank has a set bit");
        rank -= binomial(index, selected).expect("selected a valid set bit");
        mask |= 1u32 << index;
        upper = index;
    }
    mask
}

fn cardinality_bounds(size: &Range<i32>, inner_len: u32) -> Option<(u32, u32)> {
    let (min, max) = match size {
        Range::Unbounded => (0, inner_len),
        Range::Single(n) => ((*n).try_into().ok()?, (*n).try_into().ok()?),
        Range::UnboundedR(min) => ((*min).try_into().ok()?, inner_len),
        Range::UnboundedL(max) => (0, (*max).try_into().ok()?),
        Range::Bounded(min, max) => ((*min).try_into().ok()?, (*max).try_into().ok()?),
    };
    (min <= max && max <= inner_len).then_some((min, max))
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn combinatorial_ranks_round_trip() {
        for inner_len in 0..=10 {
            for min in 0..=inner_len {
                for max in min..=inner_len {
                    for mask in 0..(1u32 << inner_len) {
                        if (min..=max).contains(&mask.count_ones()) {
                            let rank = rank_mask(mask, inner_len, min);
                            assert_eq!(unrank_mask(rank, inner_len, (min, max)), mask);
                        }
                    }
                }
            }
        }
    }
}
