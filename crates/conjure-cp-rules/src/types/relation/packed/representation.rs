use crate::shared::representation_prelude::*;
use crate::types::relation::binary_attrs::binary_relation_constraints;
use conjure_cp::ast::matrix::enumerate_indices;
use conjure_cp::ast::{BinaryAttr, GroundDomain, Moo, Range, Reference, eval_constant};
use conjure_cp::{domain_int, essence_expr, into_matrix_expr, matrix_expr, range};

/// Packed masks use a signed `i32`, leaving 30 usable tuple bits -- one bit per potential tuple
/// in the relation's full column cartesian product, mirroring `SetPacked`'s bit mask over its
/// element universe. Unlike `RelationOccurrence`, columns need only be enumerable (any finite
/// domain `SetPacked` itself would also accept, including a set-typed column), not specifically
/// matrix-indexable, since packing never builds a native multi-dimensional matrix.
const MAX_TUPLE_UNIVERSE: u32 = 30;

register_representation!(
    RelationPacked("packed")
    struct State<T> {
        /// The single integer variable, domain, or literal holding the tuple-membership bit mask.
        pub packed: T,
        /// Every potential tuple in the column cartesian product, in bit-position order.
        pub elements: Moo<Vec<Vec<Literal>>>,
        /// The relation's arity (number of columns), stored directly so callers never need to
        /// infer it from `elements` (which is empty, and so arity-less, for a 0-tuple universe).
        pub arity: usize,
        /// Inclusive cardinality bounds.
        pub cardinality: (u32, u32),
        /// Number of bit masks represented by `packed`, including cardinality-invalid masks.
        pub total_size: i32,
        /// Binary-relation attributes to enforce structurally, if any. Only ever non-empty for a
        /// binary relation whose two columns have the same domain (`init` rejects any other
        /// combination).
        pub binary_attrs: Vec<BinaryAttr>,
        /// The shared column domain, present only when `binary_attrs` is non-empty.
        pub binary_domain: Option<DomainPtr>
    }
    impl State<DeclarationPtr> {
        pub fn packed_expr(&self) -> Expression {
            Reference::new(self.packed.clone()).into()
        }

        fn bit_expr(&self, index: usize) -> Expression {
            let divisor = 1i32
                .checked_shl(index as u32)
                .expect("validated packed relation bit index");
            let packed = self.packed_expr();
            essence_expr!((&packed / &divisor) % 2)
        }

        pub fn element_occurs_expr(&self, index: usize) -> Expression {
            Expression::Eq(
                Metadata::new(),
                Moo::new(self.bit_expr(index)),
                Moo::new(1.into()),
            )
        }

        fn decoded_cardinality_expr(&self) -> Expression {
            let bits = (0..self.elements.len())
                .map(|index| self.bit_expr(index))
                .collect::<Vec<_>>();
            Expression::Sum(Metadata::new(), Moo::new(into_matrix_expr!(bits)))
        }

        pub fn cardinality_expr(&self) -> Expression {
            let (min, max) = self.cardinality;
            if min == max {
                return (min as i32).into();
            }
            self.decoded_cardinality_expr()
        }

        /// `fields` in in the same order as this relation's columns; may be a mix of constant and
        /// decision-variable expressions. Fully-constant fields fast-path to a direct bit test;
        /// otherwise every candidate tuple is tried, guarded by that tuple's own occurrence bit.
        pub fn tuple_membership_expr(&self, fields: &[Expression]) -> Expression {
            let constants: Option<Vec<Literal>> = fields.iter().map(eval_constant).collect();
            if let Some(values) = constants {
                return self
                    .elements
                    .iter()
                    .position(|candidate| tuple_eq(candidate, &values))
                    .map(|index| self.element_occurs_expr(index))
                    .unwrap_or_else(|| false.into());
            }

            let choices = self
                .elements
                .iter()
                .enumerate()
                .map(|(index, candidate)| {
                    let eqs: Vec<Expression> = fields
                        .iter()
                        .zip(candidate)
                        .map(|(field, value)| {
                            Expression::Eq(
                                Metadata::new(),
                                Moo::new(field.clone()),
                                Moo::new(value.clone().into()),
                            )
                        })
                        .collect();
                    let all_match = Expression::And(Metadata::new(), Moo::new(into_matrix_expr!(eqs)));
                    Expression::And(
                        Metadata::new(),
                        Moo::new(matrix_expr![all_match, self.element_occurs_expr(index)]),
                    )
                })
                .collect();
            Expression::Or(Metadata::new(), Moo::new(into_matrix_expr!(choices)))
        }

        /// Lower equality with a relation literal to equality on the packed rank.
        pub fn equality_to_literal_expr(&self, tuples: &[Vec<Literal>]) -> Option<Expression> {
            let mut mask = 0i32;
            for tuple in tuples {
                let index = self
                    .elements
                    .iter()
                    .position(|candidate| tuple_eq(candidate, tuple))?;
                let bit = 1i32 << index;
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
                Moo::new(mask.into()),
            ))
        }
    }
    fn init(dom: DomainPtr) -> Result<State<DomainPtr>, ReprInitError> {
        let domain_err = |msg: &str| ReprInitError::UnsupportedDomain(
            dom.clone(),
            RelationPacked::NAME,
            String::from(msg),
        );
        let Some(GroundDomain::Relation(attr, inner_doms)) = dom.as_ground() else {
            return Err(domain_err("expected a ground relation domain"));
        };

        let dims = inner_doms
            .iter()
            .map(|d| d.length())
            .collect::<Result<Vec<_>, _>>()
            .map_err(|e| domain_err(&format!("could not enumerate a relation column domain: {e}")))?;
        let total_cells: u64 = dims.iter().product();
        let total_cells = u32::try_from(total_cells)
            .map_err(|_| domain_err("relation column cartesian product is too large to pack"))?;
        if total_cells > MAX_TUPLE_UNIVERSE {
            return Err(domain_err("relation column cartesian product is too large to pack"));
        }
        let cardinality = cardinality_bounds(&attr.size, total_cells)
            .ok_or_else(|| domain_err("invalid or unsupported relation cardinality"))?;
        let elements: Vec<Vec<Literal>> = enumerate_indices(inner_doms.clone()).collect();
        let total_size = 1i32
            .checked_shl(total_cells)
            .ok_or_else(|| domain_err("packed representation would overflow i32"))?;
        let packed = domain_int!(0..(total_size - 1));

        let binary_domain = if attr.binary.is_empty() {
            None
        } else {
            let [domain, codomain] = inner_doms.as_slice() else {
                return Err(domain_err(
                    "binary relation attributes (reflexive/symmetric/etc) only apply to binary relations",
                ));
            };
            if domain != codomain {
                return Err(domain_err(
                    "binary relation attributes (reflexive/symmetric/etc) require both columns to share the same domain",
                ));
            }
            Some(domain.clone().into())
        };

        Ok(State {
            packed,
            elements: Moo::new(elements),
            arity: inner_doms.len(),
            cardinality,
            total_size,
            binary_attrs: attr.binary.clone(),
            binary_domain,
        })
    }
    fn structural(state: &State<DeclarationPtr>) -> Vec<Expression> {
        let (min, max) = state.cardinality;
        let cardinality = state.decoded_cardinality_expr();
        let min = min as i32;
        let max = max as i32;
        let mut constraints = if min == max {
            vec![essence_expr!(&cardinality = &min)]
        } else {
            vec![essence_expr!(r"(&cardinality >= &min) /\ (&cardinality <= &max)")]
        };

        if let Some(binary_domain) = &state.binary_domain {
            let member = |x: &Expression, y: &Expression| {
                state.tuple_membership_expr(&[x.clone(), y.clone()])
            };
            constraints.extend(binary_relation_constraints(
                binary_domain,
                &state.binary_attrs,
                &member,
            ));
        }
        constraints
    }
    fn down(state: &State<DomainPtr>, value: Literal) -> Result<State<Literal>, ReprDownError> {
        let Literal::AbstractLiteral(AbstractLiteral::Relation(tuples)) = value else {
            return Err(ReprDownError::BadValue(value, String::from("expected a relation literal")));
        };
        let original = Literal::AbstractLiteral(AbstractLiteral::Relation(tuples.clone()));

        let mut mask = 0i32;
        for tuple in &tuples {
            let Some(index) = state
                .elements
                .iter()
                .position(|candidate| tuple_eq(candidate, tuple))
            else {
                return Err(ReprDownError::BadValue(
                    original,
                    format!("tuple {tuple:?} is outside the relation's column domains"),
                ));
            };
            let bit = 1i32 << index;
            if mask & bit != 0 {
                return Err(ReprDownError::BadValue(original, "duplicate relation tuple".to_string()));
            }
            mask |= bit;
        }
        let cardinality = mask.count_ones();
        let (min, max) = state.cardinality;
        if cardinality < min || cardinality > max {
            return Err(ReprDownError::BadValue(
                original,
                "relation cardinality is outside the domain bounds".to_string(),
            ));
        }
        Ok(State {
            packed: Literal::Int(mask),
            elements: state.elements.clone(),
            arity: state.arity,
            cardinality: state.cardinality,
            total_size: state.total_size,
            binary_attrs: state.binary_attrs.clone(),
            binary_domain: state.binary_domain.clone(),
        })
    }
    fn up(state: State<Literal>) -> Literal {
        let Literal::Int(mask) = state.packed else {
            bug!("expected an integer literal for packed relation value, got {}", state.packed);
        };
        if mask < 0 || mask >= state.total_size {
            bug!("packed relation mask {mask} is outside its representation domain");
        }
        let tuples = state
            .elements
            .iter()
            .enumerate()
            .filter(|(index, _)| mask & (1i32 << index) != 0)
            .map(|(_, tuple)| tuple.clone())
            .collect::<Vec<_>>();
        Literal::AbstractLiteral(AbstractLiteral::Relation(tuples))
    }
    fn compactness(state: &State<DomainPtr>) -> usize {
        let (min, max) = state.cardinality;
        (min..=max)
            .map(|size| binomial(state.elements.len() as u32, size))
            .fold(0usize, usize::saturating_add)
    }
);

fn tuple_eq(a: &[Literal], b: &[Literal]) -> bool {
    a.len() == b.len() && a.iter().zip(b).all(|(x, y)| x.essence_cmp(y).is_eq())
}

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
    let max = max.min(inner_len);
    (min <= max).then_some((min, max))
}
