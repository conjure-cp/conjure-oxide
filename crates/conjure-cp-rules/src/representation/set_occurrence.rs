use super::prelude::*;
use crate::utils::lit_to_bool;
use conjure_cp::ast::{Domain, GroundDomain, Moo, Reference, domains::UInt};
use conjure_cp::{essence_expr, into_matrix_expr};
use std::collections::{HashMap, VecDeque};

static MAX_SIZE_FOR_EXPLICIT: UInt = 100;

register_representation!(
    SetOccurrence
    struct State<T> {
        pub cardinality: (UInt, UInt),
        pub occurs: Moo<HashMap<Literal, T>>
    }
    fn init(dom: DomainPtr) -> Result<State<DomainPtr>, ReprInitError> {
        let domain_err = |msg: &str| ReprInitError::UnsupportedDomain(dom.clone(), SetOccurrence::NAME, String::from(msg));
        let Some(gd @ GroundDomain::Set(_attr, inner_dom)) = dom.as_ground() else {
            return Err(domain_err("expected a ground set domain"));
        };
        let cardinality @ (_, max) = gd.set_cardinality().map_err(|e| domain_err(&format!("could not enumerate set domain: {e}")))?;
        if max > MAX_SIZE_FOR_EXPLICIT {
            return Err(domain_err("set too large"))
        }
        let inner_dom_elems = inner_dom.values().map_err(|e| domain_err(&format!("could not enumerate set domain: {e}")))?;
        let occurs = Moo::new(inner_dom_elems.map(|x| (x, Domain::bool())).collect());
        Ok(State {
            occurs,
            cardinality
        })
    }
    fn structural(state: &State<DeclarationPtr>) -> Vec<Expression> {
        // TODO: could use a comprehension here instead?
        let elems: Vec<Expression> = state.occurs.values().map(|x| {
            let re = Reference::new(x.clone());
            essence_expr!(toInt(&re))
        }).collect();
        let res = Expression::Sum(Metadata::new(), Moo::new(into_matrix_expr!(elems)));
        vec![res]
    }
    fn down(state: &State<DomainPtr>, value: Literal) -> Result<State<Literal>, ReprDownError> {
        let Literal::AbstractLiteral(AbstractLiteral::Set(elems)) = value else {
            return Err(ReprDownError::BadValue(value, String::from("expected a set literal")));
        };

        let cardinality @ (min, max) = state.cardinality;
        let elems_sz = elems.len() as UInt;
        if elems_sz < min || elems_sz > max {
            return Err(ReprDownError::BadValue(AbstractLiteral::Set(elems).into(), format!("expected between {min} and {max} elements, got {}", elems_sz)));
        }

        let mut occurs: HashMap<Literal, Literal> = elems.into_iter().map(|x| (x, true.into())).collect();
        // mark all other elements as not occuring
        for lit in state.occurs.keys() {
            if !occurs.contains_key(lit) {
                occurs.insert(lit.clone(), false.into());
            }
        }

        let occurs = Moo::new(occurs);
        Ok(State { occurs, cardinality })
    }
    fn up(state: State<Literal>) -> Literal {
        let mut elems = Vec::new();
        for (k, v) in state.occurs.iter() {
            if lit_to_bool(v) {
                elems.push(k.clone());
            }
        }
        Literal::AbstractLiteral(AbstractLiteral::Set(elems))
    }
    fn repr_vars(state: &State<DeclarationPtr>) -> VecDeque<DeclarationPtr> {
        state.occurs.values().cloned().collect()
    }
);
