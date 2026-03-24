use super::prelude::*;
use conjure_cp::ast::{Domain, Range, domains::Int};
use conjure_cp::utils::BiMap;
use std::collections::VecDeque;
use std::hash::Hash;

register_representation!(
    SatIntDirect
    struct State<T: Eq + Hash> {
        // Mapping of each possible value i of the original integer x to a boolean b_i <-> (x = i)
        vals: BiMap<Int, T>
    }
    impl<T: Eq + Hash> State<T> {
        // Iterate all entries except the one corresponding to `key`
        fn others(&self, key: Int) -> impl Iterator<Item = &T> {
            self.vals.iter().filter_map(move |(k, v)| if *k == key { None } else { Some(v) })
        }
    }
    fn init(dom: DomainPtr) -> Result<State<DomainPtr>, ReprInitError> {
        let Some(rngs) = dom.as_int_ground() else {
            return Err(ReprInitError::UnsupportedDomain(dom, SatIntDirect::NAME, String::from("expected a ground int domain")));
        };
        let Some(itr) = Range::values(rngs) else {
            return Err(ReprInitError::UnsupportedDomain(dom, SatIntDirect::NAME, String::from("domain is not enumerable")));
        };
        let vals: BiMap<Int, DomainPtr> = itr.map(|v| (v, Domain::bool())).collect();
        Ok(State { vals })
    }
    fn structural(_state: &State<DeclarationPtr>) -> Vec<Expression> {
        vec![]
    }
    fn down(state: &State<DomainPtr>, value: Literal) -> Result<State<Literal>, ReprDownError> {
        todo!()
    }
    fn up(state: State<Literal>) -> Literal {
        todo!()
    }
    fn repr_vars(state: &State<DeclarationPtr>) -> VecDeque<DeclarationPtr> {
        todo!()
    }
);
