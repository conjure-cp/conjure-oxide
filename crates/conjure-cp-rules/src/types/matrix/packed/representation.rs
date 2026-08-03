use crate::shared::representation_prelude::matrix::{flatten, unflatten_matrix};
use crate::shared::representation_prelude::*;
use conjure_cp::ast::matrix::shape_of_dom;
use conjure_cp::ast::{GroundDomain, Moo, Reference};
use conjure_cp::representation::ReprInitError;
use conjure_cp::utils::{BiMap, MatrixShape};
use conjure_cp::{domain_int, essence_expr, into_matrix_expr, range};

register_representation!(
    MatrixPacked("packed")
    struct State<T> {
        /// The complete row-major matrix encoded as one mixed-radix integer.
        pub packed: T,
        pub dimensions: Vec<usize>,
        pub strides: Vec<usize>,
        pub index_domains: Vec<Moo<GroundDomain>>,
        pub indices: Moo<Vec<BiMap<usize, Literal>>>,
        /// Element values in primitive symmetry order.
        pub values: Moo<Vec<Literal>>,
        /// Place value for each flattened matrix position.
        pub places: Vec<i32>,
        pub radix: i32,
        pub total_size: i32
    }
    impl State<DeclarationPtr> {
        pub fn packed_expr(&self) -> Expression {
            Reference::new(self.packed.clone()).into()
        }

        pub fn decoded_element(&self, flat: usize) -> Expression {
            let packed = self.packed_expr();
            let place = self.places[flat];
            let digit = match (place, self.radix) {
                (_, 1) => Expression::from(0),
                (1, radix) => essence_expr!(&packed % &radix),
                (_, radix) => essence_expr!((&packed / &place) % &radix),
            };
            let values = self.values.iter().cloned().map(Expression::from).collect::<Vec<_>>();
            Expression::SafeIndex(
                Metadata::new(),
                Moo::new(into_matrix_expr!(values)),
                vec![essence_expr!(&digit + 1)],
            )
        }

        pub fn decoded_matrix(&self) -> Expression {
            let elements = (0..self.dimensions.iter().product())
                .map(|flat| self.decoded_element(flat))
                .collect::<Vec<_>>();
            let domains = self.index_domains.iter().cloned().map(DomainPtr::from).collect::<Vec<_>>();
            unflatten_matrix(&elements, &domains, &self.strides)
        }
    }
    impl<T> State<T> {
        pub fn flat_index(&self, indices: &[Literal]) -> Option<usize> {
            if indices.len() != self.indices.len() {
                return None;
            }
            indices.iter().enumerate().try_fold(0usize, |flat, (dimension, value)| {
                let offset = self.indices[dimension].get_by_right(value)?;
                flat.checked_add(offset.checked_mul(self.strides[dimension])?)
            })
        }

        pub fn encode(&self, elements: &[Literal]) -> Option<i32> {
            if elements.len() != self.places.len() {
                return None;
            }
            elements.iter().zip(&self.places).try_fold(0i32, |packed, (value, place)| {
                let digit = self.values.iter().position(|candidate| candidate.essence_cmp(value).is_eq())? as i32;
                packed.checked_add(digit.checked_mul(*place)?)
            })
        }
    }
    fn init(dom: DomainPtr) -> Result<State<DomainPtr>, ReprInitError> {
        let domain_err = |message: &str| ReprInitError::UnsupportedDomain(
            dom.clone(), MatrixPacked::NAME, message.to_owned());
        let resolved = dom.resolve().ok().ok_or_else(|| domain_err("expected a ground matrix domain"))?;
        let GroundDomain::Matrix(element_domain, _) = resolved.as_ref() else {
            return Err(domain_err("expected a matrix domain"));
        };
        let mut values = match element_domain.as_ref() {
            GroundDomain::Bool => vec![Literal::Bool(true), Literal::Bool(false)],
            GroundDomain::Int(_) => element_domain.values()
                .map_err(|error| domain_err(&format!("could not enumerate element domain: {error}")))?
                .collect::<Vec<_>>(),
            _ => return Err(domain_err("packed matrices currently require primitive bool or int elements")),
        };
        values.dedup_by(|left, right| left.essence_cmp(right).is_eq());

        let MatrixShape { size, dims, strides, idx_doms } = shape_of_dom(resolved.as_ref())
            .ok().ok_or_else(|| domain_err("could not determine finite matrix shape"))?;
        if values.is_empty() && size != 0 {
            return Err(domain_err("non-empty matrices require a non-empty element domain"));
        }
        let radix = i32::try_from(values.len().max(1))
            .map_err(|_| domain_err("element domain is too large"))?;
        let total_size = radix.checked_pow(size as u32)
            .ok_or_else(|| domain_err("packed matrix domain would overflow i32"))?;
        let mut places = vec![1i32; size];
        for position in (0..size.saturating_sub(1)).rev() {
            places[position] = places[position + 1].checked_mul(radix)
                .ok_or_else(|| domain_err("packed matrix place value would overflow i32"))?;
        }
        let indices = idx_doms.iter().map(|index_domain| {
            index_domain.values()
                .map(|values| BiMap::from_iter(values.enumerate()))
                .map_err(|error| domain_err(&format!("could not enumerate index domain: {error}")))
        }).collect::<Result<Vec<_>, _>>()?;

        Ok(State {
            packed: domain_int!(0..(total_size - 1)),
            dimensions: dims,
            strides,
            index_domains: idx_doms,
            indices: Moo::new(indices),
            values: Moo::new(values),
            places,
            radix,
            total_size,
        })
    }
    fn structural(_state: &State<DeclarationPtr>) -> Vec<Expression> {
        vec![]
    }
    fn down(state: &State<DomainPtr>, value: Literal) -> Result<State<Literal>, ReprDownError> {
        let Literal::AbstractLiteral(abstract_literal @ AbstractLiteral::Matrix(..)) = &value else {
            return Err(ReprDownError::BadValue(value, "expected a matrix literal".to_owned()));
        };
        let elements = flatten(abstract_literal).cloned().collect::<Vec<_>>();
        let packed = state.encode(&elements).ok_or_else(|| ReprDownError::BadValue(
            value.clone(), "matrix shape or element value is outside its domain".to_owned()))?;
        Ok(State {
            packed: Literal::Int(packed),
            dimensions: state.dimensions.clone(),
            strides: state.strides.clone(),
            index_domains: state.index_domains.clone(),
            indices: state.indices.clone(),
            values: state.values.clone(),
            places: state.places.clone(),
            radix: state.radix,
            total_size: state.total_size,
        })
    }
    fn up(state: State<Literal>) -> Literal {
        let Literal::Int(packed) = state.packed else {
            bug!("expected a packed matrix integer, got {}", state.packed)
        };
        let elements = state.places.iter().map(|place| {
            let digit = if state.radix == 1 { 0 } else { packed / *place % state.radix };
            state.values[digit as usize].clone()
        }).collect::<Vec<_>>();
        unflatten_matrix(&elements, &state.index_domains, &state.strides)
    }
    fn compactness(state: &State<DomainPtr>) -> usize {
        state.total_size as usize
    }
);
