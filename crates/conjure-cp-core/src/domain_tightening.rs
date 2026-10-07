//! Infer tighter declaration domains from the constraint list, before rewriting.
//!
//! The first component looks at cardinality equalities `|s| = k` (and `|s| = |t|` when `|t|` is
//! constant) and, when `s` is a sequence find, intersects that size into its domain. Indexed
//! equalities `forAll i : D . |m[i]| = k_i` over a matrix of sequences are included so that each
//! cell can later be represented as a fixed-length sequence.

use std::collections::HashMap;

use crate::ast::{
    Atom, DeclarationKind, DeclarationPtr, DomainPtr, Expression as Expr, GroundDomain, Literal,
    Metadata, Model, Moo, Name, Range,
    comprehension::{Comprehension, ComprehensionQualifier},
    eval_constant,
    matrix::shape_of_dom,
};

/// Walk the model's constraints and tighten declaration domains where a fact can be proved.
pub fn tighten_domains_from_constraints(model: &mut Model) {
    let facts = collect_sequence_size_facts(model);
    apply_sequence_size_facts(facts);
}

struct SequenceSizeFact {
    decl: DeclarationPtr,
    /// `None` for a top-level sequence; `Some(indices)` for a matrix element.
    indices: Option<Vec<Literal>>,
    size: i32,
}

fn collect_sequence_size_facts(model: &Model) -> Vec<SequenceSizeFact> {
    let mut facts = Vec::new();
    for constraint in model.constraints() {
        collect_from_expr(constraint, &mut facts);
    }
    facts
}

fn collect_from_expr(expr: &Expr, facts: &mut Vec<SequenceSizeFact>) {
    match expr {
        Expr::And(_, inner) => {
            if let Some(children) = inner.unwrap_list_ref() {
                for child in children {
                    collect_from_expr(child, facts);
                }
            } else {
                // `forAll` parses as `And(Comprehension)`, not `And` of a list.
                collect_from_expr(inner.as_ref(), facts);
            }
        }
        Expr::Comprehension(_, comprehension) => {
            collect_from_comprehension(comprehension, facts);
        }
        Expr::Eq(_, lhs, rhs) => collect_from_card_eq(lhs, rhs, facts),
        _ => {}
    }
}

fn collect_from_comprehension(comprehension: &Comprehension, facts: &mut Vec<SequenceSizeFact>) {
    let generators: Vec<&DeclarationPtr> = comprehension
        .qualifiers
        .iter()
        .filter_map(|qualifier| match qualifier {
            ComprehensionQualifier::Generator { ptr } => Some(ptr),
            ComprehensionQualifier::ExpressionGenerator { .. }
            | ComprehensionQualifier::Condition(_) => None,
        })
        .collect();

    // First component: a single generator over a finite domain, body an equality of cards.
    if generators.len() != 1 {
        collect_from_expr(&comprehension.return_expression, facts);
        return;
    }

    let generator = generators[0];
    let Some(domain) = generator.domain().and_then(|domain| domain.resolve().ok()) else {
        collect_from_expr(&comprehension.return_expression, facts);
        return;
    };
    let Ok(values) = domain.values() else {
        collect_from_expr(&comprehension.return_expression, facts);
        return;
    };
    let values: Vec<Literal> = values.collect();
    if values.is_empty() {
        return;
    }

    let Expr::Eq(_, lhs, rhs) = &comprehension.return_expression else {
        collect_from_expr(&comprehension.return_expression, facts);
        return;
    };

    for value in values {
        let Some(size) = card_eq_size_at_generator(lhs, rhs, generator, &value) else {
            continue;
        };
        let Some(subject) = card_eq_subject_at_generator(lhs, rhs, generator, &value) else {
            continue;
        };
        facts.push(subject.with_size(size));
    }
}

fn collect_from_card_eq(lhs: &Expr, rhs: &Expr, facts: &mut Vec<SequenceSizeFact>) {
    let Some(size) = card_eq_constant_size(lhs, rhs) else {
        return;
    };
    if let Some(subject) = sequence_subject(lhs).or_else(|| sequence_subject(rhs)) {
        facts.push(subject.with_size(size));
    }
}

fn card_eq_constant_size(lhs: &Expr, rhs: &Expr) -> Option<i32> {
    match (as_card(lhs), as_card(rhs)) {
        (Some(_), Some(_)) => eval_card(lhs).or_else(|| eval_card(rhs)),
        (Some(_), None) => eval_int(rhs),
        (None, Some(_)) => eval_int(lhs),
        (None, None) => None,
    }
}

fn card_eq_size_at_generator(
    lhs: &Expr,
    rhs: &Expr,
    generator: &DeclarationPtr,
    value: &Literal,
) -> Option<i32> {
    let lhs_size = eval_card_with_index(lhs, generator, value);
    let rhs_size = eval_card_with_index(rhs, generator, value);
    match (as_card(lhs), as_card(rhs), lhs_size, rhs_size) {
        (Some(_), Some(_), Some(size), _) | (Some(_), Some(_), None, Some(size)) => Some(size),
        (Some(_), None, _, _) => eval_int(rhs),
        (None, Some(_), _, _) => eval_int(lhs),
        _ => None,
    }
}

fn card_eq_subject_at_generator(
    lhs: &Expr,
    rhs: &Expr,
    generator: &DeclarationPtr,
    value: &Literal,
) -> Option<SequenceSubject> {
    let lhs_eval = eval_card_with_index(lhs, generator, value).is_some();
    let rhs_eval = eval_card_with_index(rhs, generator, value).is_some();
    match (as_card(lhs), as_card(rhs), lhs_eval, rhs_eval) {
        // Prefer the side that is *not* a known constant, so `|locs[i]| = |clues[i]|` tightens
        // the find, not the given.
        (Some(_), Some(_), false, true) => sequence_subject_at(lhs, generator, value),
        (Some(_), Some(_), true, false) => sequence_subject_at(rhs, generator, value),
        (Some(_), None, _, _) => sequence_subject_at(lhs, generator, value),
        (None, Some(_), _, _) => sequence_subject_at(rhs, generator, value),
        _ => None,
    }
}

struct SequenceSubject {
    decl: DeclarationPtr,
    indices: Option<Vec<Literal>>,
}

impl SequenceSubject {
    fn with_size(self, size: i32) -> SequenceSizeFact {
        SequenceSizeFact {
            decl: self.decl,
            indices: self.indices,
            size,
        }
    }
}

fn sequence_subject(expr: &Expr) -> Option<SequenceSubject> {
    let collection = as_card(expr)?;
    match collection {
        Expr::Atomic(_, Atom::Reference(reference)) => Some(SequenceSubject {
            decl: reference.ptr().clone(),
            indices: None,
        }),
        Expr::UnsafeIndex(_, subject, indices) | Expr::SafeIndex(_, subject, indices) => {
            let Expr::Atomic(_, Atom::Reference(reference)) = subject.as_ref() else {
                return None;
            };
            let indices: Vec<Literal> = indices.iter().map(eval_constant).collect::<Option<_>>()?;
            Some(SequenceSubject {
                decl: reference.ptr().clone(),
                indices: Some(indices),
            })
        }
        _ => None,
    }
}

fn sequence_subject_at(
    expr: &Expr,
    generator: &DeclarationPtr,
    value: &Literal,
) -> Option<SequenceSubject> {
    let collection = as_card(expr)?;
    match collection {
        Expr::Atomic(_, Atom::Reference(reference)) => Some(SequenceSubject {
            decl: reference.ptr().clone(),
            indices: None,
        }),
        Expr::UnsafeIndex(_, subject, indices) | Expr::SafeIndex(_, subject, indices) => {
            let Expr::Atomic(_, Atom::Reference(reference)) = subject.as_ref() else {
                return None;
            };
            let indices: Vec<Literal> = indices
                .iter()
                .map(|index| eval_index_at(index, generator, value))
                .collect::<Option<_>>()?;
            Some(SequenceSubject {
                decl: reference.ptr().clone(),
                indices: Some(indices),
            })
        }
        _ => None,
    }
}

fn as_card(expr: &Expr) -> Option<&Expr> {
    match expr {
        Expr::Card(_, collection) => Some(collection.as_ref()),
        _ => None,
    }
}

fn eval_card(expr: &Expr) -> Option<i32> {
    match eval_constant(expr)? {
        Literal::Int(size) if size >= 0 => Some(size),
        _ => None,
    }
}

fn eval_int(expr: &Expr) -> Option<i32> {
    match eval_constant(expr)? {
        Literal::Int(size) if size >= 0 => Some(size),
        _ => None,
    }
}

fn eval_card_with_index(expr: &Expr, generator: &DeclarationPtr, value: &Literal) -> Option<i32> {
    let collection = as_card(expr)?;
    let instantiated = instantiate_index_expr(collection, generator, value)?;
    eval_card(&Expr::Card(Metadata::new(), Moo::new(instantiated)))
}

fn instantiate_index_expr(
    expr: &Expr,
    generator: &DeclarationPtr,
    value: &Literal,
) -> Option<Expr> {
    match expr {
        Expr::UnsafeIndex(meta, subject, indices) => {
            let indices = instantiate_indices(indices, generator, value)?;
            Some(Expr::UnsafeIndex(meta.clone(), subject.clone(), indices))
        }
        Expr::SafeIndex(meta, subject, indices) => {
            let indices = instantiate_indices(indices, generator, value)?;
            Some(Expr::SafeIndex(meta.clone(), subject.clone(), indices))
        }
        _ => None,
    }
}

fn instantiate_indices(
    indices: &[Expr],
    generator: &DeclarationPtr,
    value: &Literal,
) -> Option<Vec<Expr>> {
    indices
        .iter()
        .map(|index| {
            eval_index_at(index, generator, value)
                .map(|lit| Expr::Atomic(Metadata::new(), Atom::Literal(lit)))
        })
        .collect()
}

fn eval_index_at(index: &Expr, generator: &DeclarationPtr, value: &Literal) -> Option<Literal> {
    if let Expr::Atomic(_, Atom::Reference(reference)) = index
        && reference.ptr() == generator
    {
        return Some(value.clone());
    }
    eval_constant(index)
}

fn apply_sequence_size_facts(facts: Vec<SequenceSizeFact>) {
    let mut by_decl: HashMap<Name, (DeclarationPtr, Vec<SequenceSizeFact>)> = HashMap::new();
    for fact in facts {
        if fact.size < 0 {
            continue;
        }
        let name = fact.decl.name().clone();
        by_decl
            .entry(name)
            .or_insert_with(|| (fact.decl.clone(), Vec::new()))
            .1
            .push(fact);
    }

    for (decl, decl_facts) in by_decl.into_values() {
        apply_facts_to_declaration(decl, decl_facts);
    }
}

fn apply_facts_to_declaration(mut decl: DeclarationPtr, facts: Vec<SequenceSizeFact>) {
    if !matches!(
        &*decl.kind(),
        DeclarationKind::Find(_) | DeclarationKind::FindAuxiliary(_)
    ) {
        return;
    }

    let Some(domain) = decl.domain() else {
        return;
    };
    let Ok(ground) = domain.resolve() else {
        return;
    };

    let top_level: Vec<i32> = facts
        .iter()
        .filter(|fact| fact.indices.is_none())
        .map(|fact| fact.size)
        .collect();
    let indexed: Vec<(Vec<Literal>, i32)> = facts
        .iter()
        .filter_map(|fact| {
            fact.indices
                .as_ref()
                .map(|indices| (indices.clone(), fact.size))
        })
        .collect();

    if let GroundDomain::Sequence(attr, _) = ground.as_ref()
        && let Some(&size) = top_level.first()
        && top_level.iter().all(|&other| other == size)
        && let Some(new_size) = merge_to_size(&attr.size, size)
        && let Some(new_domain) = with_sequence_size(ground.as_ref(), new_size)
        && let Some(mut var) = decl.as_find_mut()
    {
        var.domain = new_domain.into();
        return;
    }

    let GroundDomain::Matrix(inner, _) = ground.as_ref() else {
        return;
    };
    let GroundDomain::Sequence(_, _) = inner.as_ref() else {
        return;
    };
    if indexed.is_empty() {
        return;
    }

    let Ok(shape) = shape_of_dom(ground.as_ref()) else {
        return;
    };
    let Some(index_tuples) = index_tuples(&shape.idx_doms) else {
        return;
    };
    if index_tuples.len() != shape.size {
        return;
    }

    let mut size_by_index: HashMap<Vec<Literal>, i32> = HashMap::new();
    for (indices, size) in indexed {
        if let Some(existing) = size_by_index.get(&indices)
            && *existing != size
        {
            return;
        }
        size_by_index.insert(indices, size);
    }
    if size_by_index.len() != index_tuples.len() {
        return;
    }

    let sizes: Vec<i32> = index_tuples
        .iter()
        .map(|indices| size_by_index[indices])
        .collect();
    let min_size = *sizes.iter().min().unwrap_or(&0);
    let max_size = *sizes.iter().max().unwrap_or(&0);
    let Some(new_inner_size) = merge_to_bounds(
        match inner.as_ref() {
            GroundDomain::Sequence(attr, _) => &attr.size,
            _ => return,
        },
        min_size,
        max_size,
    ) else {
        return;
    };

    let element_domains: Option<Vec<DomainPtr>> = sizes
        .iter()
        .copied()
        .map(|size| {
            let size_range = merge_to_size(
                match inner.as_ref() {
                    GroundDomain::Sequence(attr, _) => &attr.size,
                    _ => return None,
                },
                size,
            )?;
            Some(with_sequence_size(inner.as_ref(), size_range)?.into())
        })
        .collect();
    let Some(element_domains) = element_domains else {
        return;
    };

    let Some(new_inner) = with_sequence_size(inner.as_ref(), new_inner_size) else {
        return;
    };
    let GroundDomain::Matrix(_, idx_doms) = ground.as_ref() else {
        return;
    };
    let new_domain = GroundDomain::Matrix(Moo::new(new_inner), idx_doms.clone());
    if let Some(mut var) = decl.as_find_mut() {
        var.domain = new_domain.into();
        var.element_domains = Some(element_domains);
    }
}

fn with_sequence_size(domain: &GroundDomain, size: Range<i32>) -> Option<GroundDomain> {
    let GroundDomain::Sequence(attr, inner) = domain else {
        return None;
    };
    let mut attr = attr.clone();
    attr.size = size;
    Some(GroundDomain::Sequence(attr, inner.clone()))
}

fn merge_to_size(current: &Range<i32>, size: i32) -> Option<Range<i32>> {
    current.contains(&size).then_some(Range::Single(size))
}

fn merge_to_bounds(current: &Range<i32>, min: i32, max: i32) -> Option<Range<i32>> {
    let lo = current.low().copied().unwrap_or(min).max(min);
    let hi = current.high().copied().unwrap_or(max).min(max);
    if lo > hi {
        return None;
    }
    Some(Range::new(Some(lo), Some(hi)))
}

fn index_tuples(idx_doms: &[Moo<GroundDomain>]) -> Option<Vec<Vec<Literal>>> {
    let lists: Vec<Vec<Literal>> = idx_doms
        .iter()
        .map(|domain| domain.values().ok().map(Iterator::collect))
        .collect::<Option<_>>()?;
    Some(lists.into_iter().fold(vec![vec![]], |prefixes, list| {
        prefixes
            .into_iter()
            .flat_map(|prefix| {
                list.iter().map(move |item| {
                    let mut next = prefix.clone();
                    next.push(item.clone());
                    next
                })
            })
            .collect()
    }))
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::ast::{
        AbstractLiteral, DeclarationPtr, Domain, JectivityAttr, Name, Reference, SequenceAttr,
        SymbolTablePtr, comprehension::ComprehensionBuilder,
    };
    use crate::{domain_int, matrix_expr, range};

    fn sequence_attr(size: Range<i32>) -> SequenceAttr {
        SequenceAttr {
            size,
            jectivity: JectivityAttr::None,
            representation: None,
        }
    }

    fn int_lit(value: i32) -> Expr {
        Expr::Atomic(Metadata::new(), Atom::Literal(Literal::Int(value)))
    }

    fn ref_expr(decl: &DeclarationPtr) -> Expr {
        Expr::Atomic(
            Metadata::new(),
            Atom::Reference(Reference::new(decl.clone())),
        )
    }

    fn card(expr: Expr) -> Expr {
        Expr::Card(Metadata::new(), Moo::new(expr))
    }

    fn eq(lhs: Expr, rhs: Expr) -> Expr {
        Expr::Eq(Metadata::new(), Moo::new(lhs), Moo::new(rhs))
    }

    fn sequence_size(decl: &DeclarationPtr) -> Range<i32> {
        let domain = decl.domain().unwrap();
        let GroundDomain::Sequence(attr, _) = domain.as_ground().unwrap() else {
            panic!("expected a sequence domain");
        };
        attr.size.clone()
    }

    #[test]
    fn tightens_a_top_level_sequence_from_a_cardinality_equality() {
        let mut model = Model::new(Default::default());
        let s = DeclarationPtr::new_find(
            Name::user("s"),
            Domain::sequence(sequence_attr(range!(0..10)), domain_int!(1..3)),
        );
        model.symbols_mut().insert(s.clone()).unwrap();
        model.add_constraint(eq(card(ref_expr(&s)), int_lit(4)));

        tighten_domains_from_constraints(&mut model);

        let s = model.symbols().lookup_local(&Name::user("s")).unwrap();
        assert_eq!(sequence_size(&s), Range::Single(4));
    }

    #[test]
    fn leaves_the_domain_alone_when_the_size_is_outside_the_declared_range() {
        let mut model = Model::new(Default::default());
        let s = DeclarationPtr::new_find(
            Name::user("s"),
            Domain::sequence(sequence_attr(range!(0..3)), domain_int!(1..3)),
        );
        model.symbols_mut().insert(s.clone()).unwrap();
        model.add_constraint(eq(card(ref_expr(&s)), int_lit(4)));

        tighten_domains_from_constraints(&mut model);

        let s = model.symbols().lookup_local(&Name::user("s")).unwrap();
        assert_eq!(sequence_size(&s), range!(0..3));
    }

    #[test]
    fn tightens_matrix_of_sequence_cells_from_a_forall_cardinality_equality() {
        let mut model = Model::new(Default::default());
        let clues_lit = Literal::AbstractLiteral(AbstractLiteral::Matrix(
            vec![
                Literal::AbstractLiteral(AbstractLiteral::Sequence(vec![
                    Literal::Int(1),
                    Literal::Int(2),
                    Literal::Int(3),
                ])),
                Literal::AbstractLiteral(AbstractLiteral::Sequence(vec![
                    Literal::Int(1),
                    Literal::Int(1),
                    Literal::Int(1),
                    Literal::Int(1),
                    Literal::Int(1),
                ])),
            ],
            Moo::new(GroundDomain::Int(vec![range!(1..2)])),
        ));
        let clues = DeclarationPtr::new_value_letting(Name::user("clues"), Expr::from(clues_lit));
        let locs = DeclarationPtr::new_find(
            Name::user("locs"),
            Domain::matrix(
                Domain::sequence(sequence_attr(range!(0..10)), domain_int!(1..9)),
                vec![domain_int!(1..2)],
            ),
        );
        model.symbols_mut().insert(clues.clone()).unwrap();
        model.symbols_mut().insert(locs.clone()).unwrap();

        let builder = ComprehensionBuilder::new(SymbolTablePtr::new());
        let i_template = DeclarationPtr::new_find(Name::user("i"), domain_int!(1..2));
        let mut builder = builder.generator(i_template);
        let i = builder
            .generator_symboltable()
            .read()
            .lookup_local(&Name::user("i"))
            .expect("generator i");
        let body = eq(
            card(Expr::UnsafeIndex(
                Metadata::new(),
                Moo::new(ref_expr(&locs)),
                vec![ref_expr(&i)],
            )),
            card(Expr::UnsafeIndex(
                Metadata::new(),
                Moo::new(ref_expr(&clues)),
                vec![ref_expr(&i)],
            )),
        );
        let comprehension = builder.with_return_value(body);
        model.add_constraint(Expr::And(
            Metadata::new(),
            Moo::new(matrix_expr![Expr::Comprehension(
                Metadata::new(),
                Moo::new(comprehension)
            )]),
        ));

        tighten_domains_from_constraints(&mut model);

        let locs = model.symbols().lookup_local(&Name::user("locs")).unwrap();
        let var = locs.as_find().unwrap();
        let domains = var.element_domains.as_ref().expect("per-cell domains");
        assert_eq!(domains.len(), 2);
        let sizes: Vec<Range<i32>> = domains
            .iter()
            .map(|domain| {
                let GroundDomain::Sequence(attr, _) = domain.as_ground().unwrap() else {
                    panic!("expected a sequence element domain");
                };
                attr.size.clone()
            })
            .collect();
        assert_eq!(sizes, vec![Range::Single(3), Range::Single(5)]);
    }
}
