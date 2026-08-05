use crate::shared::representation_prelude::*;
use conjure_cp::ast::{Domain, GroundDomain, SetAttr};

register_representation!(
    RelationAsSet("as_set")
    struct State<T> {
        /// The relation, channelled to a set of tuples. This is deliberately a minimal
        /// prerequisite slice of the future relation campaign, not a general relation
        /// representation: it only supports binary relations with no binary attributes
        /// (reflexive/symmetric/etc).
        pub set_decl: T
    }
    fn init(dom: DomainPtr) -> Result<State<DomainPtr>, ReprInitError> {
        let domain_err = |msg: &str| ReprInitError::UnsupportedDomain(
            dom.clone(),
            RelationAsSet::NAME,
            String::from(msg),
        );
        let Some(GroundDomain::Relation(attr, inner_doms)) = dom.as_ground() else {
            return Err(domain_err("expected a ground relation domain"));
        };
        if !attr.binary.is_empty() {
            return Err(domain_err(
                "as_set representation does not yet support binary relation attributes (reflexive/symmetric/etc)",
            ));
        }
        let [domain, codomain] = inner_doms.as_slice() else {
            return Err(domain_err("as_set representation currently only supports binary relations"));
        };
        let tuple_dom = Domain::tuple(vec![domain.clone().into(), codomain.clone().into()]);
        let set_decl = Domain::set(SetAttr::new(attr.size.clone()), tuple_dom);
        Ok(State { set_decl })
    }
    fn structural(_state: &State<DeclarationPtr>) -> Vec<Expression> {
        vec![]
    }
    fn down(_state: &State<DomainPtr>, value: Literal) -> Result<State<Literal>, ReprDownError> {
        let Literal::AbstractLiteral(AbstractLiteral::Relation(tuples)) = value else {
            return Err(ReprDownError::BadValue(value, String::from("expected a relation literal")));
        };
        let elems = tuples
            .into_iter()
            .map(|fields| Literal::AbstractLiteral(AbstractLiteral::Tuple(fields)))
            .collect();
        Ok(State {
            set_decl: Literal::AbstractLiteral(AbstractLiteral::Set(elems)),
        })
    }
    fn up(state: State<Literal>) -> Literal {
        let Literal::AbstractLiteral(AbstractLiteral::Set(elems)) = state.set_decl else {
            bug!("expected a relation-as-set value to be a set, got {}", state.set_decl)
        };
        let tuples = elems
            .into_iter()
            .map(|elem| {
                let Literal::AbstractLiteral(AbstractLiteral::Tuple(fields)) = elem else {
                    bug!("expected a relation-as-set element to be a tuple, got {elem}")
                };
                fields
            })
            .collect();
        Literal::AbstractLiteral(AbstractLiteral::Relation(tuples))
    }
);
