use super::prelude::*;
use conjure_cp::utils::BiMap;
use std::collections::VecDeque;
use std::hash::Hash;

register_representation!(
    RecordToAtom
    struct State<T: Eq + Hash> {
        pub elems: BiMap<Name, T>
    }
    fn init(dom: DomainPtr) -> Result<State<DomainPtr>, ReprInitError> {
        todo!()
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
