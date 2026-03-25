use super::prelude::*;
use conjure_cp::ast::Reference;
use conjure_cp::ast::records::RecordValue;
use std::collections::{HashMap, VecDeque};

register_representation!(
    RecordToAtom
    struct State<T> {
        pub elems: HashMap<Name, T>
    }
    impl State<DeclarationPtr> {
        /// Get the variable representing a field of this record
        pub fn field_ref(&self, name: &Name) -> Option<Reference> {
            self.elems.get(name).cloned().map(Reference::from)
        }
    }
    fn init(dom: DomainPtr) -> Result<State<DomainPtr>, ReprInitError> {
        let Some(ents) = dom.as_record() else {
            return Err(ReprInitError::UnsupportedDomain(dom, RecordToAtom::NAME, String::from("expected a record domain")));
        };
        let elems: HashMap<Name, DomainPtr> = ents.into_iter().map(|e| (e.name, e.domain)).collect();
        Ok(State { elems })
    }
    fn structural(_state: &State<DeclarationPtr>) -> Vec<Expression> {
        vec![]
    }
    fn down(state: &State<DomainPtr>, value: Literal) -> Result<State<Literal>, ReprDownError> {
        let Literal::AbstractLiteral(AbstractLiteral::Record(vals)) = &value else {
            return Err(ReprDownError::BadValue(value, String::from("expected a record literal")));
        };
        let elems: HashMap<Name, Literal> = vals.iter().cloned().map(|e| (e.name, e.value)).collect();
        for (k, v) in elems.iter() {
            let Some(_dom) = state.elems.get(k) else {
                return Err(ReprDownError::BadValue(value, format!("unexpected entry: `{k} = {v}`")));
            };
        }
        Ok(State { elems })
    }
    fn up(state: State<Literal>) -> Literal {
        let ents: Vec<RecordValue<Literal>> = state.elems.into_iter().map(|(name, value)| RecordValue { name, value }).collect();
        Literal::AbstractLiteral(AbstractLiteral::Record(ents))
    }
    fn repr_vars(state: &State<DeclarationPtr>) -> VecDeque<DeclarationPtr> {
        state.elems.values().cloned().collect()
    }
);
