use crate::shared::representation_prelude::*;
use conjure_cp::ast::ac_operators::ACOperatorKind;
use conjure_cp::ast::comprehension::ComprehensionBuilder;
use conjure_cp::ast::{
    Domain, GroundDomain, JectivityAttr, Moo, PartialityAttr, Range, Reference, RelAttr,
    SymbolTablePtr,
};

register_representation!(
    FunctionAsRelation("as_relation")
    struct State<T> {
        /// The function's graph, channelled to a binary relation of (key, value) pairs. Suited
        /// to sparse partial functions, since the relation's own (eventually chosen) explicit
        /// representation only stores active entries plus a marker, not one slot per domain
        /// element.
        pub relation_decl: T,
        /// Every codomain value, used by the surjective structural constraint.
        pub codomain_values: Moo<Vec<Literal>>,
        /// Jectivity to enforce structurally.
        pub jectivity: JectivityAttr
    }
    impl State<DeclarationPtr> {
        pub fn relation_expr(&self) -> Expression {
            Reference::new(self.relation_decl.clone()).into()
        }
    }
    fn init(dom: DomainPtr) -> Result<State<DomainPtr>, ReprInitError> {
        let domain_err = |msg: &str| ReprInitError::UnsupportedDomain(
            dom.clone(),
            FunctionAsRelation::NAME,
            String::from(msg),
        );
        let Some(GroundDomain::Function(attr, domain, codomain)) = dom.as_ground() else {
            return Err(domain_err("expected a ground function domain"));
        };

        // A total function's graph has exactly one entry per domain element; fixing the
        // relation's size this tightly (rather than leaving it open) is what lets a single
        // "no duplicate keys" structural constraint below double as the totality check too
        // (exactly `|domain|` distinct keys drawn from a `|domain|`-sized key domain must, by
        // pigeonhole, cover every domain element).
        let relation_size = match attr.partiality {
            PartialityAttr::Total => {
                let n = domain.length()
                    .map_err(|e| domain_err(&format!("could not determine the domain size of a total function: {e}")))?;
                let n = i32::try_from(n).map_err(|_| domain_err("function domain is too large"))?;
                Range::Single(n)
            }
            PartialityAttr::Partial => attr.size.clone(),
        };

        let codomain_values = if matches!(attr.jectivity, JectivityAttr::Surjective | JectivityAttr::Bijective) {
            codomain.values()
                .map_err(|e| domain_err(&format!("could not enumerate function codomain: {e}")))?
                .collect()
        } else {
            vec![]
        };

        let relation_decl = Domain::relation(
            RelAttr { size: relation_size, binary: vec![] },
            vec![domain.clone().into(), codomain.clone().into()],
        );

        Ok(State {
            relation_decl,
            codomain_values: Moo::new(codomain_values),
            jectivity: attr.jectivity.clone(),
        })
    }
    fn structural(state: &State<DeclarationPtr>) -> Vec<Expression> {
        let rel = state.relation_expr();

        // Every representable relation value must be a well-formed function: no two entries
        // share a key with different values.
        let mut constraints = vec![forall_pairs(&rel, |i, j| {
            Expression::Imply(
                Metadata::new(),
                Moo::new(Expression::Eq(
                    Metadata::new(),
                    Moo::new(tuple_index(i, 1)),
                    Moo::new(tuple_index(j, 1)),
                )),
                Moo::new(Expression::Eq(
                    Metadata::new(),
                    Moo::new(tuple_index(i, 2)),
                    Moo::new(tuple_index(j, 2)),
                )),
            )
        })];

        if matches!(
            state.jectivity,
            JectivityAttr::Injective | JectivityAttr::Bijective
        ) {
            constraints.push(forall_pairs(&rel, |i, j| {
                Expression::Imply(
                    Metadata::new(),
                    Moo::new(Expression::Neq(
                        Metadata::new(),
                        Moo::new(tuple_index(i, 1)),
                        Moo::new(tuple_index(j, 1)),
                    )),
                    Moo::new(Expression::Neq(
                        Metadata::new(),
                        Moo::new(tuple_index(i, 2)),
                        Moo::new(tuple_index(j, 2)),
                    )),
                )
            }));
        }

        if matches!(
            state.jectivity,
            JectivityAttr::Surjective | JectivityAttr::Bijective
        ) {
            for value in state.codomain_values.iter() {
                let value: Expression = value.clone().into();
                constraints.push(exists_in(&rel, |j| {
                    Expression::Eq(
                        Metadata::new(),
                        Moo::new(tuple_index(j, 2)),
                        Moo::new(value.clone()),
                    )
                }));
            }
        }

        constraints
    }
    fn down(state: &State<DomainPtr>, value: Literal) -> Result<State<Literal>, ReprDownError> {
        let Literal::AbstractLiteral(AbstractLiteral::Function(pairs)) = value else {
            return Err(ReprDownError::BadValue(value, String::from("expected a function literal")));
        };
        let tuples = pairs.into_iter().map(|(key, value)| vec![key, value]).collect();
        Ok(State {
            relation_decl: Literal::AbstractLiteral(AbstractLiteral::Relation(tuples)),
            codomain_values: state.codomain_values.clone(),
            jectivity: state.jectivity.clone(),
        })
    }
    fn up(state: State<Literal>) -> Literal {
        let Literal::AbstractLiteral(AbstractLiteral::Relation(tuples)) = state.relation_decl else {
            bug!("expected a function-as-relation value to be a relation, got {}", state.relation_decl)
        };
        let pairs = tuples
            .into_iter()
            .map(|fields| {
                let [key, value] = <[Literal; 2]>::try_from(fields).unwrap_or_else(|fields| {
                    bug!("expected a binary relation tuple, got {} elements", fields.len())
                });
                (key, value)
            })
            .collect();
        Literal::AbstractLiteral(AbstractLiteral::Function(pairs))
    }
);

/// A one-based index into a tuple-valued expression, e.g. `e[1]`.
fn tuple_index(e: &Expression, index: i32) -> Expression {
    Expression::SafeIndex(Metadata::new(), Moo::new(e.clone()), vec![index.into()])
}

/// Looks up a just-inserted quantified variable's declaration by name.
fn quantified_ref(symbols: &SymbolTablePtr, name: &Name) -> Expression {
    let decl = symbols
        .read()
        .lookup(name)
        .unwrap_or_else(|| bug!("expected a quantified variable {name} to be in scope"));
    Reference::new(decl).into()
}

/// `forAll i, j in &collection . body(i, j)`, built directly rather than via `essence_expr!`
/// (which cannot build quantifier/aggregate expressions: they need a symbol table for the bound
/// variable, which the macro does not provide).
fn forall_pairs(
    collection: &Expression,
    body: impl FnOnce(&Expression, &Expression) -> Expression,
) -> Expression {
    let i_name = Name::user("i");
    let j_name = Name::user("j");
    let mut builder = ComprehensionBuilder::new(SymbolTablePtr::new())
        .expression_generator(i_name.clone(), collection.clone())
        .expression_generator(j_name.clone(), collection.clone());
    let symbols = builder.return_expr_symboltable();
    let i_ref = quantified_ref(&symbols, &i_name);
    let j_ref = quantified_ref(&symbols, &j_name);
    let return_expr = body(&i_ref, &j_ref);

    let mut comprehension = builder.with_return_value(return_expr);
    comprehension.skip_operator = Some(ACOperatorKind::And);
    let wrapped = Expression::Comprehension(Metadata::new(), Moo::new(comprehension));
    Expression::And(Metadata::new(), Moo::new(wrapped))
}

/// `exists j in &collection . body(j)`, built directly for the same reason as [`forall_pairs`].
fn exists_in(collection: &Expression, body: impl FnOnce(&Expression) -> Expression) -> Expression {
    let j_name = Name::user("j");
    let mut builder = ComprehensionBuilder::new(SymbolTablePtr::new())
        .expression_generator(j_name.clone(), collection.clone());
    let symbols = builder.return_expr_symboltable();
    let j_ref = quantified_ref(&symbols, &j_name);
    let return_expr = body(&j_ref);

    let mut comprehension = builder.with_return_value(return_expr);
    comprehension.skip_operator = Some(ACOperatorKind::Or);
    let wrapped = Expression::Comprehension(Metadata::new(), Moo::new(comprehension));
    Expression::Or(Metadata::new(), Moo::new(wrapped))
}
