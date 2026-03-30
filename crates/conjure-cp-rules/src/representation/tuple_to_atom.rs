use super::prelude::*;
use conjure_cp::ast::Reference;

register_representation!(
    TupleToAtom
    struct State<T> {
        pub elems: Vec<T>
    }
    impl State<DeclarationPtr> {
        pub fn elem_refs(&self) -> impl Iterator<Item = Reference> {
            self.elems.iter().cloned().map(Reference::new)
        }
    }
    fn init(dom: DomainPtr) -> Result<State<DomainPtr>, ReprInitError> {
        let Some(elems) = dom.as_tuple() else {
            return Err(ReprInitError::UnsupportedDomain(dom, TupleToAtom::NAME, String::from("expected a tuple domain")));
        };
        Ok(State { elems })
    }
    fn structural(_state: &State<DeclarationPtr>) -> Vec<Expression> {
        vec![]
    }
    fn down(state: &State<DomainPtr>, value: Literal) -> Result<State<Literal>, ReprDownError> {
        let Literal::AbstractLiteral(AbstractLiteral::Tuple(vals)) = value else {
            return Err(ReprDownError::BadValue(value, String::from("expected a tuple literal")));
        };
        if vals.len() != state.elems.len() {
            let msg = format!("expected {} elements, got {}", state.elems.len(), vals.len());
            let val = Literal::AbstractLiteral(AbstractLiteral::Tuple(vals));
            return Err(ReprDownError::BadValue(val, msg));
        }
        Ok(State { elems: vals })
    }
    fn up(state: State<Literal>) -> Literal {
        Literal::AbstractLiteral(AbstractLiteral::Tuple(state.elems))
    }
);
