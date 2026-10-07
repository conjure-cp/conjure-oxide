use crate::shared::representation_prelude::*;
use crate::types::product::{canonical_product_literal, symmetry_values};
use conjure_cp::ast::{GroundDomain, Moo, Reference, records::Field};
use conjure_cp::representation::ReprInitError;
use conjure_cp::utils::BiMap;
use conjure_cp::{domain_int, range};

register_representation!(
    RecordPacked("packed")
    struct State<T> {
        /// The complete record encoded as one dense mixed-radix integer.
        pub packed: T,
        /// Canonical field-name order and its corresponding digit index.
        pub indices: BiMap<Name, usize>,
        /// Field values in Conjure symmetry order.
        pub values: Moo<Vec<Vec<Literal>>>,
        /// Place value for each record field.
        pub places: Vec<i32>,
        /// Number of values for each record field.
        pub radices: Vec<i32>,
        /// Number of record values represented by `packed`.
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
                    let field = canonical_product_literal(field.clone());
                    let digit = values
                        .iter()
                        .position(|candidate| candidate.essence_cmp(&field).is_eq())?;
                    packed.checked_add(i32::try_from(digit).ok()?.checked_mul(*place)?)
                },
            )
        }
    }
    fn init(dom: DomainPtr) -> Result<State<DomainPtr>, ReprInitError> {
        let domain_err = |message: &str| ReprInitError::UnsupportedDomain(
            dom.clone(), RecordPacked::NAME, message.to_owned());
        let resolved = dom.resolve().ok()
            .ok_or_else(|| domain_err("expected a ground record domain"))?;
        let GroundDomain::Record(mut fields) = resolved.as_ref().clone() else {
            return Err(domain_err("expected a record domain"));
        };
        fields.sort_by(|lhs, rhs| lhs.name.cmp(&rhs.name));

        let mut indices = BiMap::with_capacity(fields.len());
        let values = fields
            .iter()
            .enumerate()
            .map(|(index, Field { name, value })| {
                indices.insert(name.clone(), index);
                symmetry_values(value).ok_or_else(|| domain_err(&format!(
                    "field {name} is not a supported finite packed domain"
                )))
            })
            .collect::<Result<Vec<_>, _>>()?;
        if values.iter().any(Vec::is_empty) {
            return Err(domain_err("record fields must have non-empty domains"));
        }

        let radices = values
            .iter()
            .map(|values| i32::try_from(values.len()).map_err(|_| domain_err("record field domain is too large")))
            .collect::<Result<Vec<_>, _>>()?;
        let mut places = vec![1i32; radices.len()];
        for index in (0..radices.len().saturating_sub(1)).rev() {
            places[index] = places[index + 1]
                .checked_mul(radices[index + 1])
                .ok_or_else(|| domain_err("packed record place value would overflow i32"))?;
        }
        let total_size = radices.iter().try_fold(1i32, |size, radix| {
            size.checked_mul(*radix)
                .ok_or_else(|| domain_err("packed record domain would overflow i32"))
        })?;

        Ok(State {
            packed: domain_int!(0..(total_size - 1)),
            indices,
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
        let Literal::AbstractLiteral(AbstractLiteral::Record(fields)) = &value else {
            return Err(ReprDownError::BadValue(value, "expected a record literal".to_owned()));
        };
        if fields.len() != state.values.len() {
            return Err(ReprDownError::BadValue(value, "record has the wrong number of fields".to_owned()));
        }
        let mut ordered = vec![None; fields.len()];
        for Field { name, value: field } in fields {
            let Some(index) = state.indices.get_by_left(name) else {
                return Err(ReprDownError::BadValue(value.clone(), format!("unexpected record field {name}")));
            };
            ordered[*index] = Some(field.clone());
        }
        let Some(ordered) = ordered.into_iter().collect::<Option<Vec<_>>>() else {
            return Err(ReprDownError::BadValue(value, "missing record field".to_owned()));
        };
        let packed = state.encode(&ordered).ok_or_else(|| ReprDownError::BadValue(
            value.clone(), "record field value is outside its domain".to_owned()))?;
        Ok(State {
            packed: Literal::Int(packed),
            indices: state.indices.clone(),
            values: state.values.clone(),
            places: state.places.clone(),
            radices: state.radices.clone(),
            total_size: state.total_size,
        })
    }
    fn up(state: State<Literal>) -> Literal {
        let Literal::Int(packed) = state.packed else {
            bug!("expected a packed record integer, got {}", state.packed)
        };
        let fields = state.values.iter().zip(&state.places).enumerate().map(
            |(index, (values, place))| {
                let digit = packed / *place % values.len() as i32;
                let name = state.indices.get_by_right(&index).unwrap_or_else(|| {
                    bug!("packed record index {index} has no field name")
                }).clone();
                Field { name, value: values[digit as usize].clone() }
            },
        ).collect();
        Literal::AbstractLiteral(AbstractLiteral::Record(fields))
    }
    fn compactness(state: &State<DomainPtr>) -> usize {
        state.total_size as usize
    }
);
