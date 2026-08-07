//! Channels a permutation to two total, bijective `Function` declarations (forwards and
//! backwards), adapted from Conjure's `Representations/Permutation/PermutationAsFunction.hs`.
//!
//! Both inner functions reuse the already-built `FunctionExplicit`/`FunctionAsRelation` machinery
//! entirely -- declaring an aux variable with a `Function`-typed domain is enough to have it
//! recursively get its own representation chosen, exactly like a compound-codomain `values_matrix`
//! already does for `FunctionExplicit` itself.

use crate::shared::representation_prelude::*;
use crate::types::partition::common::{eq, range_body};
use crate::types::relation::binary_attrs::quantify;
use conjure_cp::ast::ac_operators::ACOperatorKind;
use conjure_cp::ast::{
    Domain, FuncAttr, GroundDomain, JectivityAttr, Moo, PartialityAttr, Range, Reference,
};
use std::collections::{HashMap, HashSet};

register_representation!(
    PermutationAsFunction("function")
    struct State<T> {
        /// The forwards mapping: `image(forwards, i)` is what the permutation sends `i` to.
        pub forwards: T,
        /// The backwards (inverse) mapping: `image(backwards, i)` is what maps to `i`.
        pub backwards: T,
        /// The permutation's inner (element) domain, shared by both inner functions' domain and
        /// codomain.
        pub inner_domain: DomainPtr,
        /// Every inner-domain value, in a fixed canonical (domain-enumeration) order -- shared by
        /// `down`/`up` so cycle reconstruction is deterministic.
        pub domain_values: Moo<Vec<Literal>>,
        /// `numMoved`'s attribute range, checked against the count of positions where `forwards`
        /// disagrees with the identity.
        pub num_moved: Range<i32>
    }
    fn init(dom: DomainPtr) -> Result<State<DomainPtr>, ReprInitError> {
        let domain_err = |msg: &str| ReprInitError::UnsupportedDomain(
            dom.clone(),
            PermutationAsFunction::NAME,
            String::from(msg),
        );
        let Some(GroundDomain::Permutation(attr, inner)) = dom.as_ground() else {
            return Err(domain_err("expected a ground permutation domain"));
        };
        let inner_domain: DomainPtr = inner.clone().into();
        let domain_values: Vec<Literal> = inner_domain.values()
            .map_err(|e| domain_err(&format!("could not enumerate permutation domain: {e}")))?
            .collect();

        let func_attr = FuncAttr::<i32> {
            size: Range::Unbounded,
            partiality: PartialityAttr::Total,
            jectivity: JectivityAttr::Bijective,
        };
        let forwards = Domain::function(func_attr.clone(), inner_domain.clone(), inner_domain.clone());
        let backwards = Domain::function(func_attr, inner_domain.clone(), inner_domain.clone());

        Ok(State {
            forwards,
            backwards,
            inner_domain,
            domain_values: Moo::new(domain_values),
            num_moved: attr.num_moved.clone(),
        })
    }
    fn structural(state: &State<DeclarationPtr>) -> Vec<Expression> {
        let forwards_ref: Expression = Reference::new(state.forwards.clone()).into();
        let backwards_ref: Expression = Reference::new(state.backwards.clone()).into();

        // numMoved = sum([ toInt(i != image(forwards, i)) | i : innerDomain ])
        let cardinality = quantify(
            &[state.inner_domain.clone()],
            &["i"],
            ACOperatorKind::Sum,
            |refs| {
                let i = &refs[0];
                let image_i = Expression::Image(
                    Metadata::new(),
                    Moo::new(forwards_ref.clone()),
                    Moo::new(i.clone()),
                );
                let moved = Expression::Neq(Metadata::new(), Moo::new(i.clone()), Moo::new(image_i));
                Expression::ToInt(Metadata::new(), Moo::new(moved))
            },
        );

        // forAll i : innerDomain . image(backwards, image(forwards, i)) = i
        let round_trip_forwards = quantify(
            &[state.inner_domain.clone()],
            &["i"],
            ACOperatorKind::And,
            |refs| {
                let i = &refs[0];
                let forward_image = Expression::Image(
                    Metadata::new(),
                    Moo::new(forwards_ref.clone()),
                    Moo::new(i.clone()),
                );
                let round_trip = Expression::Image(
                    Metadata::new(),
                    Moo::new(backwards_ref.clone()),
                    Moo::new(forward_image),
                );
                eq(&round_trip, i)
            },
        );

        // forAll i : innerDomain . image(forwards, image(backwards, i)) = i
        let round_trip_backwards = quantify(
            &[state.inner_domain.clone()],
            &["i"],
            ACOperatorKind::And,
            |refs| {
                let i = &refs[0];
                let backward_image = Expression::Image(
                    Metadata::new(),
                    Moo::new(backwards_ref.clone()),
                    Moo::new(i.clone()),
                );
                let round_trip = Expression::Image(
                    Metadata::new(),
                    Moo::new(forwards_ref.clone()),
                    Moo::new(backward_image),
                );
                eq(&round_trip, i)
            },
        );

        vec![
            range_body(&state.num_moved, &cardinality),
            round_trip_forwards,
            round_trip_backwards,
        ]
    }
    fn down(state: &State<DomainPtr>, value: Literal) -> Result<State<Literal>, ReprDownError> {
        let Literal::AbstractLiteral(AbstractLiteral::Permutation(cycles)) = value else {
            return Err(ReprDownError::BadValue(value, String::from("expected a permutation literal")));
        };

        let mut forward_map: HashMap<Literal, Literal> = HashMap::new();
        for cycle in &cycles {
            if cycle.is_empty() {
                continue;
            }
            for w in 0..cycle.len() {
                let from = cycle[w].clone();
                let to = cycle[(w + 1) % cycle.len()].clone();
                if forward_map.insert(from, to).is_some() {
                    return Err(ReprDownError::BadValue(
                        AbstractLiteral::Permutation(cycles).into(),
                        String::from("an element appears in more than one cycle"),
                    ));
                }
            }
        }

        let domain_set: HashSet<&Literal> = state.domain_values.iter().collect();
        for moved in forward_map.keys() {
            if !domain_set.contains(moved) {
                return Err(ReprDownError::BadValue(
                    AbstractLiteral::Permutation(cycles).into(),
                    String::from("permutation literal has a cycle element outside its domain"),
                ));
            }
        }

        let mut forward_pairs = Vec::with_capacity(state.domain_values.len());
        let mut backward_pairs = Vec::with_capacity(state.domain_values.len());
        for v in state.domain_values.iter() {
            let mapped = forward_map.get(v).cloned().unwrap_or_else(|| v.clone());
            forward_pairs.push((v.clone(), mapped.clone()));
            backward_pairs.push((mapped, v.clone()));
        }

        Ok(State {
            forwards: Literal::AbstractLiteral(AbstractLiteral::Function(forward_pairs)),
            backwards: Literal::AbstractLiteral(AbstractLiteral::Function(backward_pairs)),
            inner_domain: state.inner_domain.clone(),
            domain_values: state.domain_values.clone(),
            num_moved: state.num_moved.clone(),
        })
    }
    fn up(state: State<Literal>) -> Literal {
        let Literal::AbstractLiteral(AbstractLiteral::Function(pairs)) = state.forwards else {
            bug!("expected permutation forwards value to be a function, got {}", state.forwards)
        };
        let forward_map: HashMap<Literal, Literal> = pairs.into_iter().collect();

        let mut visited: HashSet<Literal> = HashSet::new();
        let mut cycles: Vec<Vec<Literal>> = Vec::new();
        for start in state.domain_values.iter() {
            if visited.contains(start) {
                continue;
            }
            let image = forward_map
                .get(start)
                .unwrap_or_else(|| bug!("permutation forwards value is missing an entry for a domain element"));
            if image == start {
                // Fixed point: omitted from cycle notation.
                visited.insert(start.clone());
                continue;
            }

            let mut cycle = vec![start.clone()];
            visited.insert(start.clone());
            let mut current = image.clone();
            while &current != start {
                visited.insert(current.clone());
                cycle.push(current.clone());
                current = forward_map
                    .get(&current)
                    .unwrap_or_else(|| bug!("permutation forwards value is missing an entry for a domain element"))
                    .clone();
            }
            cycles.push(cycle);
        }

        Literal::AbstractLiteral(AbstractLiteral::Permutation(cycles))
    }
);
