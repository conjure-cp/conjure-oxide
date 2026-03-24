use super::prelude::*;
use conjure_cp::ast::records::RecordValue;
use std::collections::{HashMap, VecDeque};

register_representation!(
    SetExplicitWithSize
    struct State<T> {
        pub elems_matrix: T,
        pub set_size: T
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
