use crate::shared::representation_prelude::*;
use conjure_cp::ast::{GroundDomain, Moo, Reference};
use conjure_cp::representation::ReprInitError;
use conjure_cp::{domain_int, range};
use itertools::Itertools;

register_representation!(
    TuplePacked("packed")
    struct State<T> {
        /// The complete tuple encoded as one mixed-radix integer.
        pub packed: T,
        /// Field values in Conjure symmetry order.
        pub values: Moo<Vec<Vec<Literal>>>,
        /// Place value for each tuple field.
        pub places: Vec<i32>,
        /// Number of values for each tuple field.
        pub radices: Vec<i32>,
        /// Number of tuple values represented by `packed`.
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
    impl<T> State<T> {
        pub fn encode(&self, fields: &[Literal]) -> Option<i32> {
            if fields.len() != self.values.len() {
                return None;
            }
            fields.iter().zip(self.values.iter()).zip(&self.places).try_fold(
                0i32,
                |packed, ((field, values), place)| {
                    let digit = values
                        .iter()
                        .position(|candidate| candidate.essence_cmp(field).is_eq())?;
                    packed.checked_add(i32::try_from(digit).ok()?.checked_mul(*place)?)
                },
            )
        }
    }
    fn init(dom: DomainPtr) -> Result<State<DomainPtr>, ReprInitError> {
        let domain_err = |message: &str| ReprInitError::UnsupportedDomain(
            dom.clone(), TuplePacked::NAME, message.to_owned());
        let resolved = dom.resolve().ok()
            .ok_or_else(|| domain_err("expected a ground tuple domain"))?;
        let GroundDomain::Tuple(fields) = resolved.as_ref() else {
            return Err(domain_err("expected a tuple domain"));
        };

        let values = fields
            .iter()
            .enumerate()
            .map(|(index, field)| {
                symmetry_values(field).ok_or_else(|| domain_err(&format!(
                    "field {index} is not a supported finite packed domain"
                )))
            })
            .collect::<Result<Vec<_>, _>>()?;
        if values.iter().any(Vec::is_empty) {
            return Err(domain_err("tuple fields must have non-empty domains"));
        }

        let radices = values
            .iter()
            .map(|values| i32::try_from(values.len()).map_err(|_| domain_err("tuple field domain is too large")))
            .collect::<Result<Vec<_>, _>>()?;
        let mut places = vec![1i32; radices.len()];
        for index in (0..radices.len().saturating_sub(1)).rev() {
            places[index] = places[index + 1]
                .checked_mul(radices[index + 1])
                .ok_or_else(|| domain_err("packed tuple place value would overflow i32"))?;
        }
        let total_size = radices.iter().try_fold(1i32, |size, radix| {
            size.checked_mul(*radix)
                .ok_or_else(|| domain_err("packed tuple domain would overflow i32"))
        })?;

        Ok(State {
            packed: domain_int!(0..(total_size - 1)),
            values: Moo::new(values),
            places,
            radices,
            total_size,
        })
    }
    fn structural(_state: &State<DeclarationPtr>) -> Vec<Expression> {
        vec![]
    }
    fn down(state: &State<DomainPtr>, value: Literal) -> Result<State<Literal>, ReprDownError> {
        let Literal::AbstractLiteral(AbstractLiteral::Tuple(fields)) = &value else {
            return Err(ReprDownError::BadValue(value, "expected a tuple literal".to_owned()));
        };
        let packed = state.encode(fields).ok_or_else(|| ReprDownError::BadValue(
            value.clone(), "tuple shape or field value is outside its domain".to_owned()))?;
        Ok(State {
            packed: Literal::Int(packed),
            values: state.values.clone(),
            places: state.places.clone(),
            radices: state.radices.clone(),
            total_size: state.total_size,
        })
    }
    fn up(state: State<Literal>) -> Literal {
        let Literal::Int(packed) = state.packed else {
            bug!("expected a packed tuple integer, got {}", state.packed)
        };
        let fields = state.values.iter().zip(&state.places).map(|(values, place)| {
            let digit = packed / *place % values.len() as i32;
            values[digit as usize].clone()
        }).collect();
        Literal::AbstractLiteral(AbstractLiteral::Tuple(fields))
    }
    fn compactness(state: &State<DomainPtr>) -> usize {
        state.total_size as usize
    }
);

/// Enumerate a field in the same order used by Conjure's recursive tuple symmetry ordering.
fn symmetry_values(domain: &GroundDomain) -> Option<Vec<Literal>> {
    match domain {
        GroundDomain::Bool => Some(vec![Literal::Bool(true), Literal::Bool(false)]),
        GroundDomain::Int(_) => domain.values().ok().map(Iterator::collect),
        GroundDomain::Tuple(fields) => {
            let fields = fields
                .iter()
                .map(|field| symmetry_values(field))
                .collect::<Option<Vec<_>>>()?;
            Some(
                fields
                    .into_iter()
                    .multi_cartesian_product()
                    .map(|fields| Literal::AbstractLiteral(AbstractLiteral::Tuple(fields)))
                    .collect(),
            )
        }
        _ => None,
    }
}
