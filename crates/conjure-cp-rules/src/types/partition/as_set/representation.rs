use crate::shared::representation_prelude::*;
use crate::types::partition::common::resolve_partition_size_attrs;
use conjure_cp::ast::ac_operators::ACOperatorKind;
use conjure_cp::ast::comprehension::ComprehensionBuilder;
use conjure_cp::ast::{Domain, GroundDomain, Moo, Reference, SetAttr, SymbolTablePtr};

register_representation!(
    PartitionAsSet("as_set")
    struct State<T> {
        /// The partition, channelled to a set of parts, each part itself a set of elements.
        pub set_decl: T,
        /// The partition's inner (element) domain, used to build the "every element of the inner
        /// domain appears in exactly one part" structural constraint.
        pub inner_domain: DomainPtr,
        /// The domain of a single part (`set (partSize) of innerDomain`), i.e. `set_decl`'s own
        /// element domain. Used to quantify directly over "one part" with a domain-based
        /// generator inside `exactlyOnce`'s nested sum, since nested comprehensions are Uniplate
        /// leaves in this codebase -- an inner comprehension's own `ExpressionGenerator` (e.g.
        /// `j <- set_ref`) is never visited by the generic rewrite engine while nested inside an
        /// outer comprehension's return expression, so it can never be lowered to a domain
        /// generator the normal way. Building it as a domain generator from the start (mirroring
        /// `forall_exists` in `types/relation/binary_attrs.rs`) sidesteps that entirely.
        pub part_domain: DomainPtr,
        /// Whether every part must have equal cardinality.
        pub is_regular: bool,
        /// Whether `partSize` already fixes an exact part size, in which case `regular` is
        /// automatically implied and stating it again would be redundant (mirrors Conjure's own
        /// `fixedPartSize` guard in `PartitionAsSet.hs`).
        pub fixed_part_size: bool
    }
    impl State<DeclarationPtr> {
        /// Lower equality between a partition and a partition literal to set equality, leaving
        /// whichever set representation `set_decl` ends up with to finish the job.
        pub fn equality_to_literal_expr(&self, parts: &[Vec<Literal>]) -> Expression {
            let elems = parts
                .iter()
                .map(|part| Literal::AbstractLiteral(AbstractLiteral::Set(part.clone())))
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
            PartitionAsSet::NAME,
            String::from(msg),
        );
        let Some(GroundDomain::Partition(attr, inner)) = dom.as_ground() else {
            return Err(domain_err("expected a ground partition domain"));
        };
        let inner_domain: DomainPtr = inner.clone().into();
        // Both numParts and partSize are bounded above by the inner domain's own size (you can
        // have at most one part per element, and no part can exceed the whole domain), falling
        // back to that bound whenever the attribute itself leaves size unbounded -- mirrors
        // Conjure's own `getMaxNumParts`/`getMaxPartSizes` (`Representations/Partition/
        // Occurrence.hs`), which fall back to `domainSizeOf` the same way. Without this, an
        // unbounded inner set domain (e.g. `partition (numParts 2) from int(1..6)`, which leaves
        // partSize unbounded) makes representation selection for that inner set never settle.
        let max_size = inner_domain
            .length_signed()
            .map_err(|_| domain_err("inner domain must have a known finite size"))?;
        let (num_parts, part_len, fixed_part_size) = resolve_partition_size_attrs(attr, max_size);
        let part_domain = Domain::set(SetAttr::new(part_len), inner_domain.clone());
        let set_decl = Domain::set(SetAttr::new(num_parts), part_domain.clone());
        Ok(State {
            set_decl,
            inner_domain,
            part_domain,
            is_regular: attr.is_regular,
            fixed_part_size,
        })
    }
    fn structural(state: &State<DeclarationPtr>) -> Vec<Expression> {
        let set_ref: Expression = Reference::new(state.set_decl.clone()).into();

        let mut constraints = vec![
            exactly_once_expr(&state.inner_domain, &state.part_domain, &set_ref),
            parts_arent_empty_expr(&set_ref),
        ];

        if state.is_regular && !state.fixed_part_size {
            constraints.push(regular_expr(&set_ref));
        }

        constraints
    }
    fn down(state: &State<DomainPtr>, value: Literal) -> Result<State<Literal>, ReprDownError> {
        let Literal::AbstractLiteral(AbstractLiteral::Partition(parts)) = value else {
            return Err(ReprDownError::BadValue(value, String::from("expected a partition literal")));
        };
        let elems = parts
            .into_iter()
            .map(|part| Literal::AbstractLiteral(AbstractLiteral::Set(part)))
            .collect();
        Ok(State {
            set_decl: Literal::AbstractLiteral(AbstractLiteral::Set(elems)),
            inner_domain: state.inner_domain.clone(),
            part_domain: state.part_domain.clone(),
            is_regular: state.is_regular,
            fixed_part_size: state.fixed_part_size,
        })
    }
    fn up(state: State<Literal>) -> Literal {
        let Literal::AbstractLiteral(AbstractLiteral::Set(elems)) = state.set_decl else {
            bug!("expected a partition-as-set value to be a set, got {}", state.set_decl)
        };
        let parts = elems
            .into_iter()
            .map(|elem| {
                let Literal::AbstractLiteral(AbstractLiteral::Set(part)) = elem else {
                    bug!("expected a partition-as-set element to be a set, got {elem}")
                };
                part
            })
            .collect();
        Literal::AbstractLiteral(AbstractLiteral::Partition(parts))
    }
);

/// Looks up a just-inserted quantified variable's declaration by name.
fn quantified_ref(symbols: &SymbolTablePtr, name: &Name) -> Expression {
    let decl = symbols
        .read()
        .lookup(name)
        .unwrap_or_else(|| bug!("expected a quantified variable {name} to be in scope"));
    Reference::new(decl).into()
}

/// `forAll j <- &collection . body(j)`.
fn forall_over(
    collection: &Expression,
    body: impl FnOnce(&Expression) -> Expression,
) -> Expression {
    let j_name = Name::user("j");
    let mut builder = ComprehensionBuilder::new(SymbolTablePtr::new())
        .expression_generator(j_name.clone(), collection.clone());
    let symbols = builder.return_expr_symboltable();
    let j_ref = quantified_ref(&symbols, &j_name);
    let return_expr = body(&j_ref);
    let mut comprehension = builder.with_return_value(return_expr);
    comprehension.skip_operator = Some(ACOperatorKind::And);
    let wrapped = Expression::Comprehension(Metadata::new(), Moo::new(comprehension));
    Expression::And(Metadata::new(), Moo::new(wrapped))
}

/// `forAll i, j <- &collection . body(i, j)`.
fn forall_pairs_over(
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

/// Every element of `inner_domain` appears in exactly one part: `forAll i : innerDomain . 1 =
/// sum([1 | j : partDomain, j in parts, i in j])`. Mirrors Conjure's `exactlyOnce`
/// (`PartitionAsSet.hs`).
///
/// The inner sum comprehension's own generator for `j` is built directly over `part_domain` (a
/// domain-based `Generator`, guarded by an explicit `j in set_ref` membership condition) rather
/// than as an `ExpressionGenerator` sourced from `set_ref` the way this file's other, non-nested
/// `forall_*`/`exists_over`-style helpers do: `Comprehension` is a Uniplate leaf in this codebase,
/// so a nested comprehension's own qualifiers are never independently visited by the generic
/// rewrite engine (e.g. `lower_set_expression_generator`) while nested inside an outer
/// comprehension's return expression -- an inner `ExpressionGenerator` built that way can never be
/// lowered, and native comprehension expansion panics on it once the outer forall reaches it
/// ("should not be called on comprehensions containing ExpressionGenerator"). Building the inner
/// generator directly from a domain sidesteps this entirely, mirroring `forall_exists` in
/// `types/relation/binary_attrs.rs`, whose own inner generator is domain-based for the same
/// reason (there, both quantifiers already share the same known domain from the start).
fn exactly_once_expr(
    inner_domain: &DomainPtr,
    part_domain: &DomainPtr,
    set_ref: &Expression,
) -> Expression {
    let i_name = Name::user("i");
    let mut outer_builder = ComprehensionBuilder::new(SymbolTablePtr::new()).generator(
        DeclarationPtr::new_find(i_name.clone(), inner_domain.clone()),
    );
    let outer_symbols = outer_builder.generator_symboltable();
    let i_ref = quantified_ref(&outer_symbols, &i_name);

    let j_name = Name::user("j");
    let mut inner_builder = ComprehensionBuilder::new(outer_symbols).generator(
        DeclarationPtr::new_find(j_name.clone(), part_domain.clone()),
    );
    let inner_symbols = inner_builder.return_expr_symboltable();
    let j_ref = quantified_ref(&inner_symbols, &j_name);
    let in_parts = Expression::In(
        Metadata::new(),
        Moo::new(j_ref.clone()),
        Moo::new(set_ref.clone()),
    );
    let contains_i = Expression::In(Metadata::new(), Moo::new(i_ref), Moo::new(j_ref));
    let inner_builder = inner_builder.guard(in_parts).guard(contains_i);
    let one = Expression::Atomic(Metadata::new(), Atom::Literal(Literal::Int(1)));
    let mut inner_comprehension = inner_builder.with_return_value(one);
    inner_comprehension.skip_operator = Some(ACOperatorKind::Sum);
    let inner_wrapped = Expression::Comprehension(Metadata::new(), Moo::new(inner_comprehension));
    let count = Expression::Sum(Metadata::new(), Moo::new(inner_wrapped));

    let one = Expression::Atomic(Metadata::new(), Atom::Literal(Literal::Int(1)));
    let return_expr = Expression::Eq(Metadata::new(), Moo::new(one), Moo::new(count));
    let mut outer_comprehension = outer_builder.with_return_value(return_expr);
    outer_comprehension.skip_operator = Some(ACOperatorKind::And);
    let outer_wrapped = Expression::Comprehension(Metadata::new(), Moo::new(outer_comprehension));
    Expression::And(Metadata::new(), Moo::new(outer_wrapped))
}

/// No part is empty: `forAll j <- parts . |j| >= 1`. Mirrors Conjure's `partsAren'tEmpty`.
fn parts_arent_empty_expr(set_ref: &Expression) -> Expression {
    forall_over(set_ref, |j| {
        let one = Expression::Atomic(Metadata::new(), Atom::Literal(Literal::Int(1)));
        Expression::Geq(
            Metadata::new(),
            Moo::new(Expression::Card(Metadata::new(), Moo::new(j.clone()))),
            Moo::new(one),
        )
    })
}

/// Every part has the same cardinality: `forAll i, j <- parts . |i| = |j|`. Mirrors Conjure's
/// `regular` (only emitted when the part size isn't already fixed by `partSize`).
fn regular_expr(set_ref: &Expression) -> Expression {
    forall_pairs_over(set_ref, |i, j| {
        Expression::Eq(
            Metadata::new(),
            Moo::new(Expression::Card(Metadata::new(), Moo::new(i.clone()))),
            Moo::new(Expression::Card(Metadata::new(), Moo::new(j.clone()))),
        )
    })
}
