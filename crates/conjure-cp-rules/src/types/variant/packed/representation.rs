use crate::shared::representation_prelude::*;
use crate::types::product::{canonical_product_literal, symmetry_values};
use conjure_cp::ast::{GroundDomain, Moo, Reference, records::Field};
use conjure_cp::representation::ReprInitError;
use conjure_cp::utils::BiMap;
use conjure_cp::{domain_int, range};

register_representation!(
    VariantPacked("packed")
    struct State<T> {
        /// The complete variant encoded as one disjoint-union rank.
        pub packed: T,
        /// Alternative names in declaration order.
        pub indices: BiMap<Name, usize>,
        /// Inner values in each alternative's symmetry order.
        pub values: Moo<Vec<Vec<Literal>>>,
        /// First packed rank assigned to each alternative.
        pub offsets: Vec<i32>,
        /// Total number of variant values.
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
        pub fn encode(&self, name: &Name, value: &Literal) -> Option<i32> {
            let index = *self.indices.get_by_left(name)?;
            let value = canonical_product_literal(value.clone());
            let digit = self.values[index]
                .iter()
                .position(|candidate| candidate.essence_cmp(&value).is_eq())?;
            self.offsets[index].checked_add(i32::try_from(digit).ok()?)
        }
    }
    fn init(dom: DomainPtr) -> Result<State<DomainPtr>, ReprInitError> {
        let domain_err = |message: String| ReprInitError::UnsupportedDomain(
            dom.clone(), VariantPacked::NAME, message);
        let resolved = dom.resolve().ok()
            .ok_or_else(|| domain_err("expected a ground variant domain".to_owned()))?;
        let GroundDomain::Variant(fields) = resolved.as_ref() else {
            return Err(domain_err("expected a variant domain".to_owned()));
        };
        if fields.is_empty() {
            return Err(domain_err("variant domains must contain an alternative".to_owned()));
        }

        let mut indices = BiMap::with_capacity(fields.len());
        let mut values = Vec::with_capacity(fields.len());
        let mut offsets = Vec::with_capacity(fields.len());
        let mut total_size = 0i32;
        for (index, Field { name, value }) in fields.iter().enumerate() {
            let field_values = symmetry_values(value).ok_or_else(|| {
                domain_err(format!("alternative {name} is not a supported finite packed domain"))
            })?;
            if field_values.is_empty() {
                return Err(domain_err(format!("alternative {name} has an empty domain")));
            }
            indices.insert(name.clone(), index);
            offsets.push(total_size);
            total_size = total_size
                .checked_add(i32::try_from(field_values.len()).map_err(|_| {
                    domain_err(format!("alternative {name} is too large"))
                })?)
                .ok_or_else(|| domain_err("packed variant domain would overflow i32".to_owned()))?;
            values.push(field_values);
        }

        Ok(State {
            packed: domain_int!(0..(total_size - 1)),
            indices,
            values: Moo::new(values),
            offsets,
            total_size,
        })
    }
    fn structural(_state: &State<DeclarationPtr>) -> Vec<Expression> {
        vec![]
    }
    fn down(state: &State<DomainPtr>, value: Literal) -> Result<State<Literal>, ReprDownError> {
        let Literal::AbstractLiteral(AbstractLiteral::Variant(field)) = &value else {
            return Err(ReprDownError::BadValue(value, "expected a variant literal".to_owned()));
        };
        let packed = state.encode(&field.name, &field.value).ok_or_else(|| {
            ReprDownError::BadValue(value.clone(), "variant value is outside its domain".to_owned())
        })?;
        Ok(State {
            packed: Literal::Int(packed),
            indices: state.indices.clone(),
            values: state.values.clone(),
            offsets: state.offsets.clone(),
            total_size: state.total_size,
        })
    }
    fn up(state: State<Literal>) -> Literal {
        let Literal::Int(packed) = state.packed else {
            bug!("expected a packed variant integer, got {}", state.packed)
        };
        let index = state.offsets.iter().enumerate().rfind(|(_, offset)| **offset <= packed)
            .map(|(index, _)| index)
            .unwrap_or_else(|| bug!("packed variant rank {packed} is outside its representation"));
        let digit = usize::try_from(packed - state.offsets[index]).unwrap();
        let value = state.values[index].get(digit).unwrap_or_else(|| {
            bug!("packed variant rank {packed} is outside alternative {index}")
        }).clone();
        let name = state.indices.get_by_right(&index).unwrap_or_else(|| {
            bug!("packed variant alternative {index} has no name")
        }).clone();
        Literal::AbstractLiteral(AbstractLiteral::Variant(Moo::new(Field { name, value })))
    }
    fn compactness(state: &State<DomainPtr>) -> usize {
        state.total_size as usize
    }
);
