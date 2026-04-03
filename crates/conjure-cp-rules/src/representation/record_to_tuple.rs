use super::prelude::*;
use conjure_cp::ast::records::RecordValue;
use conjure_cp::ast::{Domain, Reference};
use conjure_cp::utils::BiMap;

register_representation!(
    RecordToTuple
    struct State<T> {
        // 0-based indices corresponding to each field
        pub indices: BiMap<Name, usize>,
        pub tuple: T
    }
    impl State<DeclarationPtr> {
        /// Convert record index to a tuple indexing expression
        pub fn name_to_idx_expr(&self, name: &Name) -> Option<Expression> {
            // adjust for 1-based indexing in Essence
            let idx = *(self.indices.get_by_left(name)?) + 1;
            let re = Reference::new(self.tuple.clone());
            Some(Expression::SafeIndex(Metadata::new(), re.into(), vec![idx.into()]))
        }
    }
    fn init(dom: DomainPtr) -> Result<State<DomainPtr>, ReprInitError> {
        let Some(mut ents) = dom.as_record() else {
            return Err(ReprInitError::UnsupportedDomain(dom, RecordToTuple::NAME, String::from("expected a record domain")));
        };
        ents.sort();
        let mut indices = BiMap::<Name, usize>::with_capacity(ents.len());
        let mut domains = Vec::<DomainPtr>::with_capacity(ents.len());
        for (i, RecordValue { name, value }) in ents.into_iter().enumerate() {
            indices.insert(name, i);
            domains.push(value);
        }

        let tuple = Domain::tuple(domains);
        Ok(State { indices, tuple })
    }
    fn structural(_state: &State<DeclarationPtr>) -> Vec<Expression> {
        vec![]
    }
    fn down(state: &State<DomainPtr>, lit: Literal) -> Result<State<Literal>, ReprDownError> {
        let lit2 = lit.clone();
        let down_err = |msg: &str| ReprDownError::BadValue(lit2, msg.into());

        let Literal::AbstractLiteral(AbstractLiteral::Record(vals)) = lit else {
            return Err(down_err("expected a record literal"));
        };

        let len = vals.len();
        let mut tuple_elems = vec![None; len];
        for RecordValue { name, value } in vals.into_iter() {
            let Some(idx) = state.indices.get_by_left(&name).copied() else {
                return Err(down_err(&format!("unexpected entry: {name} = {value}")));
            };
            tuple_elems[idx] = Some(value);
        }

        let elems: Vec<Literal> = tuple_elems.into_iter().filter_map(|x| x).collect();
        if elems.len() != len {
            return Err(down_err("wrong number of entries"));
        }

        let tuple = Literal::AbstractLiteral(AbstractLiteral::Tuple(elems));
        Ok(State { tuple, indices: state.indices.clone()})
    }
    fn up(state: State<Literal>) -> Literal {
        let Literal::AbstractLiteral(AbstractLiteral::Tuple(elems)) = state.tuple else {
            bug!("representation variable must be a tuple")
        };
        let ents = elems.into_iter().enumerate().map(|(i, value)| {
            let name = state.indices.get_by_right(&i).unwrap().clone();
            RecordValue { name, value }
        }).collect();
        Literal::AbstractLiteral(AbstractLiteral::Record(ents))
    }
);
