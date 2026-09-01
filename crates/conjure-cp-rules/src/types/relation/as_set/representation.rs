use crate::shared::representation_prelude::*;
use crate::types::relation::binary_attrs::{binary_relation_constraints, tuple2};
use conjure_cp::ast::{BinaryAttr, Domain, GroundDomain, Moo, Reference, SetAttr};

register_representation!(
    RelationAsSet("as_set")
    struct State<T> {
        /// The relation, channelled to a set of tuples. Any arity is supported; binary
        /// relations additionally support binary attributes (reflexive/symmetric/etc) when both
        /// columns share the same domain.
        pub set_decl: T,
        /// Binary-relation attributes to enforce structurally, if any. Only ever non-empty for a
        /// binary relation whose two columns have the same domain (`init` rejects any other
        /// combination).
        pub binary_attrs: Vec<BinaryAttr>,
        /// The shared column domain, present only when `binary_attrs` is non-empty.
        pub binary_domain: Option<DomainPtr>
    }
    impl State<DeclarationPtr> {
        /// Lower equality between a relation and a relation literal to set equality, leaving
        /// whichever set representation `set_decl` ends up with to finish the job.
        pub fn equality_to_literal_expr(&self, tuples: &[Vec<Literal>]) -> Expression {
            let elems = tuples
                .iter()
                .map(|fields| Literal::AbstractLiteral(AbstractLiteral::Tuple(fields.clone())))
                .collect();
            let set_lit = Literal::AbstractLiteral(AbstractLiteral::Set(elems));
            Expression::Eq(
                Metadata::new(),
                Moo::new(Reference::new(self.set_decl.clone()).into()),
                Moo::new(set_lit.into()),
            )
        }
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
        let binary_domain = if attr.binary.is_empty() {
            None
        } else {
            let [domain, codomain] = inner_doms.as_slice() else {
                return Err(domain_err(
                    "binary relation attributes (reflexive/symmetric/etc) only apply to binary relations",
                ));
            };
            if domain != codomain {
                return Err(domain_err(
                    "binary relation attributes (reflexive/symmetric/etc) require both columns to share the same domain",
                ));
            }
            Some(DomainPtr::from(domain.clone()))
        };
        let tuple_doms = inner_doms.iter().map(|d| d.clone().into()).collect();
        let tuple_dom = Domain::tuple(tuple_doms);
        let set_decl = Domain::set(SetAttr::new(attr.size.clone()), tuple_dom);
        Ok(State { set_decl, binary_attrs: attr.binary.clone(), binary_domain })
    }
    fn structural(state: &State<DeclarationPtr>) -> Vec<Expression> {
        let Some(binary_domain) = &state.binary_domain else {
            return vec![];
        };
        let set_ref: Expression = Reference::new(state.set_decl.clone()).into();
        let member = |x: &Expression, y: &Expression| {
            Expression::In(Metadata::new(), Moo::new(tuple2(x, y)), Moo::new(set_ref.clone()))
        };
        binary_relation_constraints(binary_domain, &state.binary_attrs, &member)
    }
    fn down(state: &State<DomainPtr>, value: Literal) -> Result<State<Literal>, ReprDownError> {
        let Literal::AbstractLiteral(AbstractLiteral::Relation(tuples)) = value else {
            return Err(ReprDownError::BadValue(value, String::from("expected a relation literal")));
        };
        let elems = tuples
            .into_iter()
            .map(|fields| Literal::AbstractLiteral(AbstractLiteral::Tuple(fields)))
            .collect();
        Ok(State {
            set_decl: Literal::AbstractLiteral(AbstractLiteral::Set(elems)),
            binary_attrs: state.binary_attrs.clone(),
            binary_domain: state.binary_domain.clone(),
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
