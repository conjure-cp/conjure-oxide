use super::prelude::*;
use conjure_cp::ast::{Domain, GroundDomain, Reference, domains::Int};
use conjure_cp::{domain_int, essence_expr};

register_representation!(
    SetOccurrence
    struct State<T> {
        inner_dom_elems: Vec<Literal>,
        occurrence_matrix: T
    }
    fn init(dom: DomainPtr) -> Result<State<DomainPtr>, ReprInitError> {
        let domain_err = |msg: &str| ReprInitError::UnsupportedDomain(dom.clone(), SetOccurrence::NAME, String::from(msg));
        let Some(gd @ GroundDomain::Set(_attr, inner_dom)) = dom.as_ground() else {
            return Err(domain_err("expected a ground set domain"));
        };
        todo!()
    }
    fn structural(state: &State<DeclarationPtr>) -> Vec<Expression> {
        todo!()
    }
    fn down(state: &State<DomainPtr>, value: Literal) -> Result<State<Literal>, ReprDownError> {
        todo!()
    }
    fn up(state: State<Literal>) -> Literal {
        todo!()
    }
);
