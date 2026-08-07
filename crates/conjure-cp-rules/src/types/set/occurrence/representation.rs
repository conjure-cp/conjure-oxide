use crate::shared::representation_prelude::*;
use conjure_cp::ast::{Domain, GroundDomain, Moo, Range, Reference};
use conjure_cp::{essence_expr, into_matrix_expr, matrix_expr};
use std::collections::VecDeque;

const MAX_INNER_DOMAIN_SIZE: u64 = 100;

register_representation!(
    SetOccurrence("occurrence")
    struct State<T> {
        pub cardinality: (i32, i32),
        pub occurs: Moo<Vec<(Literal, T)>>
    }
    impl State<DeclarationPtr> {
        pub fn cardinality_expr(&self) -> Expression {
            let elems = self
                .occurs
                .iter()
                .map(|(_, declaration)| {
                    let reference = Reference::new(declaration.clone());
                    essence_expr!(toInt(&reference))
                })
                .collect();
            Expression::Sum(Metadata::new(), Moo::new(into_matrix_expr!(elems)))
        }

        pub fn membership_expr(&self, member: Expression) -> Expression {
            let choices = self
                .occurs
                .iter()
                .map(|(value, declaration)| {
                    let value_matches = Expression::Eq(
                        Metadata::new(),
                        Moo::new(member.clone()),
                        Moo::new(value.clone().into()),
                    );
                    let occurs = Reference::new(declaration.clone()).into();
                    Expression::And(
                        Metadata::new(),
                        Moo::new(matrix_expr![value_matches, occurs]),
                    )
                })
                .collect();
            Expression::Or(Metadata::new(), Moo::new(into_matrix_expr!(choices)))
        }

        /// Lower `self ⊆ superset` to implications over occurrence bits.
        ///
        /// For each domain element `e`: `occurs[e] → e ∈ superset`. This is the vertical
        /// form of Conjure's horizontal `forAll i in A . i in B` once A is occurrence.
        pub fn subset_expr(&self, superset: Expression) -> Expression {
            let constraints = self
                .occurs
                .iter()
                .map(|(value, declaration)| {
                    let occurs: Expression = Reference::new(declaration.clone()).into();
                    let membership = Expression::In(
                        Metadata::new(),
                        Moo::new(value.clone().into()),
                        Moo::new(superset.clone()),
                    );
                    Expression::Or(
                        Metadata::new(),
                        Moo::new(matrix_expr![
                            Expression::Not(Metadata::new(), Moo::new(occurs)),
                            membership,
                        ]),
                    )
                })
                .collect::<Vec<_>>();
            Expression::And(Metadata::new(), Moo::new(into_matrix_expr![constraints]))
        }

        /// Lower equality with a set literal to per-element occurrence equalities.
        pub fn equality_to_literal_expr(&self, elems: &[Literal]) -> Expression {
            let constraints = self
                .occurs
                .iter()
                .map(|(value, declaration)| {
                    let should_occur = elems.iter().any(|elem| elem == value);
                    Expression::Eq(
                        Metadata::new(),
                        Moo::new(Reference::new(declaration.clone()).into()),
                        Moo::new(should_occur.into()),
                    )
                })
                .collect::<Vec<_>>();
            Expression::And(Metadata::new(), Moo::new(into_matrix_expr![constraints]))
        }

        /// Symmetry-ordering vector used by Conjure for occurrence sets: `[-toInt(bit)]`.
        pub fn symmetry_ordering_expr(&self) -> Expression {
            let entries = self
                .occurs
                .iter()
                .map(|(_, declaration)| {
                    let bit: Expression = Reference::new(declaration.clone()).into();
                    essence_expr!(-(toInt(&bit)))
                })
                .collect::<Vec<_>>();
            into_matrix_expr!(entries)
        }
    }
    fn init(dom: DomainPtr) -> Result<State<DomainPtr>, ReprInitError> {
        let domain_err = |msg: &str| ReprInitError::UnsupportedDomain(
            dom.clone(),
            SetOccurrence::NAME,
            String::from(msg),
        );
        let Some(GroundDomain::Set(attr, inner_dom)) = dom.as_ground() else {
            return Err(domain_err("expected a ground set domain"));
        };

        let inner_len = inner_dom
            .length()
            .map_err(|e| domain_err(&format!("could not enumerate set domain: {e}")))?;
        if inner_len > MAX_INNER_DOMAIN_SIZE {
            return Err(domain_err("set inner domain is too large"));
        }
        let cardinality = cardinality_bounds(&attr.size, inner_len)
            .ok_or_else(|| domain_err("invalid or unsupported set cardinality"))?;
        let inner_dom_elems = inner_dom
            .values()
            .map_err(|e| domain_err(&format!("could not enumerate set domain: {e}")))?;
        let occurs = Moo::new(inner_dom_elems.map(|x| (x, Domain::bool())).collect());
        Ok(State { occurs, cardinality })
    }
    fn structural(state: &State<DeclarationPtr>) -> Vec<Expression> {
        let (min, max) = state.cardinality;
        let count = state.cardinality_expr();
        if min == max {
            vec![essence_expr!(&count = &min)]
        } else {
            vec![essence_expr!(r"(&count >= &min) /\ (&count <= &max)")]
        }
    }
    fn down(state: &State<DomainPtr>, value: Literal) -> Result<State<Literal>, ReprDownError> {
        let Literal::AbstractLiteral(AbstractLiteral::Set(elems)) = value else {
            return Err(ReprDownError::BadValue(value, String::from("expected a set literal")));
        };

        let cardinality @ (min, max) = state.cardinality;
        let elems_sz = elems.len() as i32;
        if elems_sz < min || elems_sz > max {
            return Err(ReprDownError::BadValue(
                AbstractLiteral::Set(elems).into(),
                format!("expected between {min} and {max} elements, got {elems_sz}"),
            ));
        }

        let mut occurs: Vec<(Literal, Literal)> =
            elems.into_iter().map(|x| (x, true.into())).collect();
        for (lit, _) in state.occurs.iter() {
            if !occurs.iter().any(|(value, _)| value == lit) {
                occurs.push((lit.clone(), false.into()));
            }
        }
        occurs.sort_by_key(|(value, _)| value.to_string());

        Ok(State {
            occurs: Moo::new(occurs),
            cardinality,
        })
    }
    fn up(state: State<Literal>) -> Literal {
        let mut elems: Vec<_> = state
            .occurs
            .iter()
            .filter_map(|(value, occurs)| match occurs {
                Literal::Bool(true) | Literal::Int(1) => Some(value.clone()),
                Literal::Bool(false) | Literal::Int(0) => None,
                other => bug!("expected a Boolean occurrence value, got {other}"),
            })
            .collect();
        elems.sort_by_key(ToString::to_string);
        Literal::AbstractLiteral(AbstractLiteral::Set(elems))
    }
    fn repr_vars(state: &State<DeclarationPtr>) -> VecDeque<DeclarationPtr> {
        state
            .occurs
            .iter()
            .map(|(_, declaration)| declaration.clone())
            .collect()
    }
    fn compactness(state: &State<DomainPtr>) -> usize {
        state
            .occurs
            .iter()
            .map(|(_, domain)| conjure_cp::representation::default_impls::domain_size(domain))
            .fold(1usize, usize::saturating_mul)
    }
);

fn cardinality_bounds(size: &Range<i32>, inner_len: u64) -> Option<(i32, i32)> {
    let inner_len = i32::try_from(inner_len).ok()?;
    let (min, max) = match size {
        Range::Unbounded => (0, inner_len),
        Range::Single(n) => (*n, *n),
        Range::UnboundedR(min) => (*min, inner_len),
        Range::UnboundedL(max) => (0, *max),
        Range::Bounded(min, max) => (*min, *max),
    };
    // Clamp oversized attributes (e.g. maxSize 3 of int(1..2)) to the inner domain.
    let max = max.min(inner_len);
    (0 <= min && min <= max).then_some((min, max))
}
