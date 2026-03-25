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
        let domain_err = |msg: &str| ReprInitError::UnsupportedDomain(dom.clone(), SetExplicitWithSize::NAME, String::from(msg));
        let Some((attr, inner_dom)) = dom.as_set_ground() else {
            return Err(domain_err("expected a ground domain"));
        };
        let Ok(inner_dom_sz) = inner_dom.length() else {
            return Err(domain_err("expected inner domain to be enumerable"));
        };

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
