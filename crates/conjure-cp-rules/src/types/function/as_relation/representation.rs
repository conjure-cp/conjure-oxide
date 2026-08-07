use crate::shared::representation_prelude::*;
use conjure_cp::ast::ac_operators::ACOperatorKind;
use conjure_cp::ast::comprehension::ComprehensionBuilder;
use conjure_cp::ast::{
    Domain, GroundDomain, JectivityAttr, Moo, PartialityAttr, Range, Reference, RelAttr,
    SymbolTablePtr,
};
use conjure_cp::{domain_int, range};

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
        /// A witness matrix of (key, value) tuples indexed by codomain value, present only for
        /// surjective/bijective functions: `witness_matrix[y]` must be a member of the relation
        /// whose second field is `y`, proving `y` is covered. Encoding surjectivity this way
        /// (introducing the function's inverse as an explicit decision variable, matching a
        /// standard CP idiom) avoids quantifying "exists a matching entry" over the relation's
        /// own decision-variable content, which needs a fresh, unbounded auxiliary witness per
        /// codomain value with no natural place in the search order. Each witness is a full
        /// tuple (not just the key) so it goes through ordinary tuple representation selection,
        /// the same way the relation's own set-of-tuples elements do, rather than being an
        /// ad-hoc literal built from mismatched parts.
        pub witness_matrix: Option<T>,
        /// A witness matrix of (key, value) tuples indexed by plain `int(1..n)` position over the
        /// function's domain (in `domain_values` order, not the domain's own type -- see
        /// `FunctionExplicit::values_matrix`'s field doc for why position is the one indexing scheme
        /// that always works), present only for total functions with a scalar domain (see
        /// `forward_values_matrix`'s field doc for why). `forward_witness_matrix[i]` must be
        /// a member of the relation, and its first field must equal `domain_values[i]`, which pins
        /// its second field to `image(f, domain_values[i])` (the relation has exactly one entry per
        /// domain value, by the totality/well-formedness constraints above). Every use of this field
        /// indexes it with a compile-time-constant position -- see `forward_values_matrix`'s field doc
        /// for why `image(f, arg)` itself must not.
        pub forward_witness_matrix: Option<T>,
        /// The value half of each `forward_witness_matrix[i]` entry, mirrored into its own plain
        /// (non-tuple) matrix and pinned equal by `structural()`. `image(f, arg)` lowers to
        /// `forward_values_matrix[elementId(domain_values_matrix, arg)]` -- an `elementId`-indexed
        /// lookup, the same way `FunctionExplicit::values_matrix` does, since `arg` need not be a
        /// compile-time literal.
        ///
        /// This mirroring exists because indexing a *compound-element* (tuple) matrix by a
        /// non-constant position doesn't work: an earlier version of this rule indexed
        /// `forward_witness_matrix` directly by `elementId(...)` and dropped the second field, which
        /// the Minion backend cannot turn into a per-field `Element` constraint chain -- it silently
        /// produced a reference to a tuple declaration's un-decomposed name, which Minion then
        /// rejected as undefined. A later attempt built the witness tuple inline (as an
        /// `AbstractLiteral::Tuple` expression, skipping `forward_witness_matrix` as a stored
        /// declaration entirely) to sidestep needing a scalar mirror at all, but that broke
        /// `RelationOccurrence`/`RelationPacked`'s own membership rules, which index straight into
        /// `member` (`SafeIndex(member, [i])`) expecting a reference backed by a real declaration --
        /// there is no rule that indexes a raw inline tuple literal, so the `SafeIndex` itself never
        /// reduced. Keeping `forward_witness_matrix` as a real declaration (indexed only by constant
        /// positions, the one shape every relation representation's membership rule already handles
        /// correctly) and mirroring only the scalar value half for `image` avoids both problems.
        ///
        /// `forward_witness_matrix`/`forward_values_matrix` are only built for a *scalar* (bool/int)
        /// domain: `forward_witness_matrix[i] in relation` still needs the relation's own membership
        /// rule to work with `domain_values[i]` as one of the tuple's fields, and `RelationOccurrence`
        /// represents membership as an occurrence-matrix lookup indexed by each field's own value --
        /// which only makes sense for a scalar field. A compound (tuple/record/...) domain value has
        /// no single Minion atom to index by; `RelationOccurrence`'s own membership rule ends up
        /// building an `elementId`-style search whose target is an un-decomposed tuple literal, which
        /// Minion rejects the same way. This is a latent gap in `RelationOccurrence`
        /// (`types/relation/occurrence/vertical/membership.rs`)/`RelationPacked`'s own membership
        /// rules for compound-typed columns, not specific to this representation -- flagged
        /// separately rather than fixed here, since every actual user of `forward_witness_matrix`
        /// (permutations, whose inner domain is always a scalar int range) is unaffected by scoping
        /// around it.
        pub forward_values_matrix: Option<T>,
        /// Every domain value, in the same order used to index `forward_witness_matrix` and
        /// `forward_values_matrix`.
        pub domain_values: Moo<Vec<Literal>>,
        /// Jectivity to enforce structurally.
        pub jectivity: JectivityAttr
    }
    impl State<DeclarationPtr> {
        pub fn relation_expr(&self) -> Expression {
            Reference::new(self.relation_decl.clone()).into()
        }

        /// The `forward_witness_matrix` entry at a one-based `domain_values` position, or `None` if
        /// this function is partial (no forward witness matrix at all).
        pub fn forward_witness_expr(&self, index: i32) -> Option<Expression> {
            let matrix = self.forward_witness_matrix.as_ref()?;
            Some(Expression::SafeIndex(
                Metadata::new(),
                Moo::new(Reference::new(matrix.clone()).into()),
                vec![index.into()],
            ))
        }

        /// The `forward_values_matrix` entry at a one-based `domain_values` position, or `None` if
        /// this function is partial (no forward values matrix at all).
        pub fn forward_value_expr(&self, index: i32) -> Option<Expression> {
            let matrix = self.forward_values_matrix.as_ref()?;
            Some(Expression::SafeIndex(
                Metadata::new(),
                Moo::new(Reference::new(matrix.clone()).into()),
                vec![index.into()],
            ))
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

        let surjective = matches!(attr.jectivity, JectivityAttr::Surjective | JectivityAttr::Bijective);
        let codomain_values = if surjective {
            codomain.values()
                .map_err(|e| domain_err(&format!("could not enumerate function codomain: {e}")))?
                .collect()
        } else {
            vec![]
        };
        let witness_tuple_dom = Domain::tuple(vec![domain.clone().into(), codomain.clone().into()]);
        let witness_matrix = if surjective {
            Some(Domain::matrix(witness_tuple_dom.clone(), vec![codomain.clone().into()]))
        } else {
            None
        };

        // Scoped to a scalar (bool/int) domain: forward_witness_matrix's structural constraint
        // proves `image(f, domain_values[i])` by asserting `(domain_values[i], value) in relation`,
        // and when the relation ends up RelationOccurrence-represented, that membership check needs
        // to index the occurrence matrix's domain dimension by `domain_values[i]` -- which only
        // works for a scalar value; a compound (tuple/record/...) domain value has no single Minion
        // atom to index by, and the resulting `elementId`-style search ends up handing Minion an
        // un-decomposed tuple literal as a search target, which it rejects. Every actual user of
        // this (permutations, whose inner domain is always a scalar int range) is unaffected;
        // compound-domain functions simply don't get a forward witness matrix or `image` support
        // under `FunctionAsRelation` yet.
        let domain_is_scalar = matches!(domain.as_ref(), GroundDomain::Bool | GroundDomain::Int(_));
        let domain_values: Vec<Literal> = if matches!(attr.partiality, PartialityAttr::Total) && domain_is_scalar {
            domain.values()
                .map_err(|e| domain_err(&format!("could not enumerate function domain: {e}")))?
                .collect()
        } else {
            vec![]
        };
        let (forward_witness_matrix, forward_values_matrix) = if matches!(attr.partiality, PartialityAttr::Total) && domain_is_scalar {
            let n = domain_values.len() as i32;
            (
                Some(Domain::matrix(witness_tuple_dom, vec![domain_int!(1..n)])),
                Some(Domain::matrix(codomain.clone().into(), vec![domain_int!(1..n)])),
            )
        } else {
            (None, None)
        };

        let relation_decl = Domain::relation(
            RelAttr { size: relation_size, binary: vec![] },
            vec![domain.clone().into(), codomain.clone().into()],
        );

        Ok(State {
            relation_decl,
            codomain_values: Moo::new(codomain_values),
            witness_matrix,
            forward_witness_matrix,
            forward_values_matrix,
            domain_values: Moo::new(domain_values),
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

        if let Some(witness_matrix) = &state.witness_matrix {
            for value in state.codomain_values.iter() {
                let value_expr: Expression = value.clone().into();
                let witness_ref = Expression::SafeIndex(
                    Metadata::new(),
                    Moo::new(Reference::new(witness_matrix.clone()).into()),
                    vec![value_expr.clone()],
                );
                constraints.push(Expression::In(
                    Metadata::new(),
                    Moo::new(witness_ref.clone()),
                    Moo::new(rel.clone()),
                ));
                constraints.push(Expression::Eq(
                    Metadata::new(),
                    Moo::new(tuple_index(&witness_ref, 2)),
                    Moo::new(value_expr),
                ));
            }
        }

        if state.forward_witness_matrix.is_some() {
            for (i, value) in state.domain_values.iter().enumerate() {
                let index = i as i32 + 1;
                let witness_ref = state
                    .forward_witness_expr(index)
                    .unwrap_or_else(|| bug!("forward_witness_matrix checked Some above"));
                let value_expr = state
                    .forward_value_expr(index)
                    .unwrap_or_else(|| bug!("forward_values_matrix checked Some above"));
                constraints.push(Expression::In(
                    Metadata::new(),
                    Moo::new(witness_ref.clone()),
                    Moo::new(rel.clone()),
                ));
                constraints.push(Expression::Eq(
                    Metadata::new(),
                    Moo::new(tuple_index(&witness_ref, 1)),
                    Moo::new(value.clone().into()),
                ));
                constraints.push(Expression::Eq(
                    Metadata::new(),
                    Moo::new(tuple_index(&witness_ref, 2)),
                    Moo::new(value_expr),
                ));
            }
        }

        constraints
    }
    fn down(state: &State<DomainPtr>, value: Literal) -> Result<State<Literal>, ReprDownError> {
        let Literal::AbstractLiteral(AbstractLiteral::Function(pairs)) = value else {
            return Err(ReprDownError::BadValue(value, String::from("expected a function literal")));
        };

        let witness_matrix = if let Some(witness_matrix) = &state.witness_matrix {
            let mut witnesses = Vec::with_capacity(state.codomain_values.len());
            for target in state.codomain_values.iter() {
                let witness_key = pairs
                    .iter()
                    .find(|(_, v)| v == target)
                    .map(|(k, _)| k.clone())
                    .ok_or_else(|| {
                        ReprDownError::BadValue(
                            AbstractLiteral::Function(pairs.clone()).into(),
                            format!("function is not surjective: no key maps to {target}"),
                        )
                    })?;
                witnesses.push(Literal::AbstractLiteral(AbstractLiteral::Tuple(vec![
                    witness_key,
                    target.clone(),
                ])));
            }
            let index_dom = match witness_matrix.as_ref() {
                conjure_cp::ast::Domain::Ground(gd) => match gd.as_ref() {
                    GroundDomain::Matrix(_, idx) => idx[0].clone(),
                    _ => bug!("expected the witness matrix to be a ground matrix domain"),
                },
                _ => bug!("expected the witness matrix domain to be ground"),
            };
            Some(Literal::from(into_matrix![witnesses; index_dom]))
        } else {
            None
        };

        let mut forward_witness_matrix = None;
        let mut forward_values_matrix = None;
        if let Some(fwm) = &state.forward_witness_matrix {
            let mut witnesses = Vec::with_capacity(state.domain_values.len());
            let mut values = Vec::with_capacity(state.domain_values.len());
            for key in state.domain_values.iter() {
                let value = pairs
                    .iter()
                    .find(|(k, _)| k == key)
                    .map(|(_, v)| v.clone())
                    .ok_or_else(|| {
                        ReprDownError::BadValue(
                            AbstractLiteral::Function(pairs.clone()).into(),
                            format!("function is not total: no value for key {key}"),
                        )
                    })?;
                witnesses.push(Literal::AbstractLiteral(AbstractLiteral::Tuple(vec![
                    key.clone(),
                    value.clone(),
                ])));
                values.push(value);
            }
            let index_dom = match fwm.as_ref() {
                conjure_cp::ast::Domain::Ground(gd) => match gd.as_ref() {
                    GroundDomain::Matrix(_, idx) => idx[0].clone(),
                    _ => bug!("expected the forward witness matrix to be a ground matrix domain"),
                },
                _ => bug!("expected the forward witness matrix domain to be ground"),
            };
            forward_witness_matrix = Some(Literal::from(into_matrix![witnesses; index_dom.clone()]));
            forward_values_matrix = Some(Literal::from(into_matrix![values; index_dom]));
        }

        let tuples = pairs.into_iter().map(|(key, value)| vec![key, value]).collect();
        Ok(State {
            relation_decl: Literal::AbstractLiteral(AbstractLiteral::Relation(tuples)),
            codomain_values: state.codomain_values.clone(),
            witness_matrix,
            forward_witness_matrix,
            forward_values_matrix,
            domain_values: state.domain_values.clone(),
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
