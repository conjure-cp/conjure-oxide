use super::prelude::*;
use conjure_cp::ast::{Domain, GroundDomain, Range, Reference};
use conjure_cp::{domain_int, essence_expr, range};

register_representation!(
    SetExplicitWithSize
    struct State<T> {
        pub cardinality: (i32, i32),
        pub elems_matrix: T,
        pub set_size: T,
        pub padding: Literal
    }
    fn init(dom: DomainPtr) -> Result<State<DomainPtr>, ReprInitError> {
        let domain_err = |msg: &str| ReprInitError::UnsupportedDomain(
            dom.clone(),
            SetExplicitWithSize::NAME,
            String::from(msg),
        );
        let Some(GroundDomain::Set(attr, inner_dom)) = dom.as_ground() else {
            return Err(domain_err("expected a ground set domain"));
        };
        let inner_len = inner_dom
            .length()
            .map_err(|e| domain_err(&format!("could not enumerate set domain: {e}")))?;
        let cardinality @ (min, max) = cardinality_bounds(&attr.size, inner_len)
            .ok_or_else(|| domain_err("invalid or unsupported set cardinality"))?;
        if max == 0 {
            return Err(domain_err("explicit representation does not support an always-empty set"));
        }
        let padding = inner_dom
            .values()
            .map_err(|e| domain_err(&format!("could not enumerate set domain: {e}")))?
            .next()
            .ok_or_else(|| domain_err("set inner domain is empty"))?;
        let set_size = domain_int!(min..max);
        let elems_matrix = Domain::matrix(inner_dom.clone().into(), vec![domain_int!(1..max)]);
        Ok(State {
            elems_matrix,
            set_size,
            cardinality,
            padding,
        })
    }
    fn structural(state: &State<DeclarationPtr>) -> Vec<Expression> {
        let (_, max) = state.cardinality;
        let _elems = Reference::from(state.elems_matrix.clone());
        let _size = Reference::from(state.set_size.clone());
        (2..=max)
            .map(|_i| {
                let _prev = _i - 1;
                essence_expr!(r"(&_size < &_i) \/ (&_elems[&_prev] <lex &_elems[&_i])")
            })
            .collect()
    }
    fn down(state: &State<DomainPtr>, value: Literal) -> Result<State<Literal>, ReprDownError> {
        let Literal::AbstractLiteral(AbstractLiteral::Set(mut elems)) = value else {
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

        elems.sort_by_key(ToString::to_string);
        elems.resize(max as usize, state.padding.clone());
        Ok(State {
            cardinality,
            set_size: Literal::from(elems_sz),
            elems_matrix: Literal::from(into_matrix!(elems)),
            padding: state.padding.clone(),
        })
    }
    fn up(state: State<Literal>) -> Literal {
        let Literal::AbstractLiteral(AbstractLiteral::Matrix(mut elems, _)) = state.elems_matrix else {
            bug!("expected set elements to be a matrix, got {}", state.elems_matrix)
        };
        let Literal::Int(set_size) = state.set_size else {
            bug!("expected set size to be an integer, got {}", state.set_size)
        };
        elems.truncate(set_size as usize);
        Literal::AbstractLiteral(AbstractLiteral::Set(elems))
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
    (0 <= min && min <= max && max <= inner_len).then_some((min, max))
}
