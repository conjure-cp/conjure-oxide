use super::prelude::*;
use conjure_cp::ast::{Domain, GroundDomain, Reference, domains::Int};
use conjure_cp::{domain_int, essence_expr};

register_representation!(
    SetExplicitWithSize
    struct State<T> {
        pub cardinality: (Int, Int),
        pub elems_matrix: T,
        pub set_size: T
    }
    fn init(dom: DomainPtr) -> Result<State<DomainPtr>, ReprInitError> {
        let domain_err = |msg: &str| ReprInitError::UnsupportedDomain(dom.clone(), SetExplicitWithSize::NAME, String::from(msg));
        let Some(gd @ GroundDomain::Set(_attr, inner_dom)) = dom.as_ground() else {
            return Err(domain_err("expected a ground set domain"));
        };
        let cardinality @ (min, max) = gd.set_cardinality_signed().map_err(|e| domain_err(&format!("could not enumerate set domain: {e}")))?;
        let set_size = domain_int!(min..max);
        let elems_matrix = Domain::matrix(inner_dom.into(), vec![domain_int!(1..max)]);
        Ok(State { elems_matrix, set_size, cardinality })
    }
    fn structural(state: &State<DeclarationPtr>) -> Vec<Expression> {
        let (_, max) = state.cardinality;
        let mut res = Vec::with_capacity(max as usize);
        let re = Reference::from(state.elems_matrix.clone());
        for i in 2..max {
            let prev = i - 1;
            res.push(essence_expr!(&re[&prev] <lex &re[&i]));
        }
        res
    }
    fn down(state: &State<DomainPtr>, value: Literal) -> Result<State<Literal>, ReprDownError> {
        let Literal::AbstractLiteral(AbstractLiteral::Set(elems)) = value else {
            return Err(ReprDownError::BadValue(value, String::from("expected a set literal")));
        };

        let cardinality @ (min, max) = state.cardinality;
        let elems_sz = elems.len() as Int;
        if elems_sz < min || elems_sz > max {
            return Err(ReprDownError::BadValue(AbstractLiteral::Set(elems).into(), format!("expected between {min} and {max} elements, got {}", elems_sz)));
        }

        let set_size = Literal::from(elems_sz);
        let elems_matrix = Literal::from(into_matrix!(elems));
        Ok(State { cardinality, set_size, elems_matrix })
    }
    fn up(state: State<Literal>) -> Literal {
        let Literal::AbstractLiteral(AbstractLiteral::Matrix(elems, _)) = state.elems_matrix else {
            bug!("expected set elements to be a matrix, got {}", state.elems_matrix)
        };
        Literal::AbstractLiteral(AbstractLiteral::Set(elems))
    }
);
