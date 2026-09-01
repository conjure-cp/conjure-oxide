//! Representation-independent horizontal rules for partitions, mirroring Conjure's
//! `Rules/Horizontal/Partition.hs`. Every rule here reduces to `parts(p)`/`participants(p)`,
//! which each partition representation implements via its own vertical comprehension rule (e.g.
//! `PartitionAsSet`'s `lower_partition_as_set_expression_generator`), or -- for `participants`/
//! `party`, which only make sense when iterated -- expands the comprehension generator directly
//! into a `parts(p)`-sourced one, matching Conjure's own restriction that these two operators are
//! only usable as a comprehension generator's source.

use conjure_cp::ast::{
    Atom, DeclarationPtr, Expression as Expr, Metadata, Moo, Name, Reference, ReturnType,
    SymbolTable, Typeable, comprehension::ComprehensionQualifier,
};
use conjure_cp::rule_engine::{
    ApplicationError::RuleNotApplicable, ApplicationResult, RuleEffect, register_rule,
};
use uniplate::{Biplate, Uniplate};

fn is_partition(e: &Expr) -> bool {
    matches!(e.return_type(), ReturnType::Partition(_))
}

/// `x = y` for partitions ⟺ `parts(x) = parts(y)`; the existing generic set-equality rule
/// finishes the job once both sides are set-typed. Mirrors Conjure's `partition-eq`.
#[register_rule("Base", 8700, [Eq])]
fn partition_eq(expr: &Expr, _: &SymbolTable) -> ApplicationResult {
    let Expr::Eq(_, x, y) = expr else {
        return Err(RuleNotApplicable);
    };
    if !is_partition(x) || !is_partition(y) {
        return Err(RuleNotApplicable);
    }
    Ok(RuleEffect::pure(Expr::Eq(
        Metadata::new(),
        Moo::new(Expr::Parts(Metadata::new(), x.clone())),
        Moo::new(Expr::Parts(Metadata::new(), y.clone())),
    )))
}

/// `x != y` for partitions ⟺ `parts(x) != parts(y)`. Mirrors Conjure's `partition-neq`.
#[register_rule("Base", 8700, [Neq])]
fn partition_neq(expr: &Expr, _: &SymbolTable) -> ApplicationResult {
    let Expr::Neq(_, x, y) = expr else {
        return Err(RuleNotApplicable);
    };
    if !is_partition(x) || !is_partition(y) {
        return Err(RuleNotApplicable);
    }
    Ok(RuleEffect::pure(Expr::Neq(
        Metadata::new(),
        Moo::new(Expr::Parts(Metadata::new(), x.clone())),
        Moo::new(Expr::Parts(Metadata::new(), y.clone())),
    )))
}

/// `|p| = |participants(p)|`. Mirrors Conjure's `partition-card`.
#[register_rule("Base", 8700, [Card])]
fn partition_card(expr: &Expr, _: &SymbolTable) -> ApplicationResult {
    let Expr::Card(_, collection) = expr else {
        return Err(RuleNotApplicable);
    };
    if !is_partition(collection) {
        return Err(RuleNotApplicable);
    }
    Ok(RuleEffect::pure(Expr::Card(
        Metadata::new(),
        Moo::new(Expr::Participants(Metadata::new(), collection.clone())),
    )))
}

/// `x in p` ⟺ `x in parts(p)`: `x` is checked against the partition's parts directly. Mirrors
/// Conjure's `partition-in`.
#[register_rule("Base", 8700, [In])]
fn partition_in(expr: &Expr, _: &SymbolTable) -> ApplicationResult {
    let Expr::In(_, x, p) = expr else {
        return Err(RuleNotApplicable);
    };
    if !is_partition(p) {
        return Err(RuleNotApplicable);
    }
    Ok(RuleEffect::pure(Expr::In(
        Metadata::new(),
        x.clone(),
        Moo::new(Expr::Parts(Metadata::new(), p.clone())),
    )))
}

/// `together`/`apart` are deliberately **not** implemented yet.
///
/// The natural formula (`exists`/`forAll i <- parts(p) . elements subsetEq i`, built the same way
/// as this file's other comprehension-based rules) type-checks and looks correct in the rule
/// trace, but produces wrong solutions once solved: after `i <- parts(p)` is lowered to a
/// domain-based generator and native comprehension expansion substitutes each candidate part into
/// the *return expression* (`elements subsetEq i`, e.g. `1 in i /\ 2 in i`), that per-branch
/// membership check is silently dropped from the final constraint -- every candidate part survives
/// in the expanded disjunction/conjunction, not just the ones actually satisfying the check
/// (confirmed empirically: solving `together({1,2}, x)` for `x : partition (numParts 2, partSize
/// 3) from int(1..6)` returns all 10 unconstrained partitions instead of the 4 where 1 and 2 share
/// a part). This looks like a real gap in how `expand_native`'s per-substitution return-expression
/// simplification interacts with a *lowered* expression generator's guard/return-expression split
/// specifically (Conjure's own `forall_exists`-shaped nested comprehensions, which use a
/// *domain-based* inner generator instead, don't hit it) -- root-causing and fixing it properly
/// needs more investigation than this campaign step's scope. Declining outright here means using
/// `together`/`apart` fails loudly ("model feature not supported") rather than silently
/// mistranslating, matching this codebase's established precedent for known gaps (e.g.
/// `AttributeAsConstraint`'s Function jectivity/totality fallback).
#[register_rule("Base", 8700, [Together])]
fn partition_together(_expr: &Expr, _: &SymbolTable) -> ApplicationResult {
    Err(RuleNotApplicable)
}

/// See [`partition_together`]'s doc comment: `apart` shares the same not-yet-implemented status
/// and the same underlying gap.
#[register_rule("Base", 8700, [Apart])]
fn partition_apart(_expr: &Expr, _: &SymbolTable) -> ApplicationResult {
    Err(RuleNotApplicable)
}

/// `j <- participants(p)` ⟺ `i <- parts(p), j <- i`: flatten every part's elements. `participants`
/// only makes sense when iterated, matching Conjure's own restriction (`rule_Participants` in
/// `Horizontal/Partition.hs` only matches inside a comprehension generator).
///
/// Known gap: reaching this rule at all currently needs `p <- parts(x)`/`p <- participants(x)`
/// (an *expression*-sourced comprehension generator over one of these two operators) to parse
/// successfully in the first place, which -- separately from this rule's own logic -- hits parser-
/// level errors for some constraint shapes around it (observed for `and([ |p| = 2 | p <- parts(x)
/// ])` and `and([ p subsetEq p | p <- parts(x) ])`, root cause not yet identified). This rule is
/// still correct and kept for when that parser gap is fixed; direct (non-generator) uses of
/// `parts(p)`/`participants(p)`, e.g. `|parts(p)| = 3`, are unaffected and already verified working.
#[register_rule("Base", 8700, [Comprehension])]
fn expand_participants_comprehension_generator(expr: &Expr, _: &SymbolTable) -> ApplicationResult {
    let Expr::Comprehension(metadata, comprehension) = expr else {
        return Err(RuleNotApplicable);
    };

    let Some((index, old_ptr, partition)) =
        comprehension
            .qualifiers
            .iter()
            .enumerate()
            .find_map(|(index, qualifier)| {
                let ComprehensionQualifier::ExpressionGenerator { ptr } = qualifier else {
                    return None;
                };
                let source = (*ptr.as_quantified_expr()?).clone();
                let Expr::Participants(_, partition) = source else {
                    return None;
                };
                Some((index, ptr.clone(), (*partition).clone()))
            })
    else {
        return Err(RuleNotApplicable);
    };

    let mut comprehension = comprehension.as_ref().clone();
    comprehension.symbols = comprehension.symbols.detach();

    let parts_expr = Expr::Parts(Metadata::new(), Moo::new(partition));
    let i_ptr = DeclarationPtr::new_quantified_expr(Name::user("i"), parts_expr);
    comprehension.symbols.write().update_insert(i_ptr.clone());
    let i_ref = Expr::from(Reference::new(i_ptr.clone()));

    let j_ptr = DeclarationPtr::new_quantified_expr(old_ptr.name().clone(), i_ref);
    comprehension.symbols.write().update_insert(j_ptr.clone());
    let j_ref = Expr::from(Reference::new(j_ptr.clone()));

    comprehension.return_expression =
        replace_reference(comprehension.return_expression, &old_ptr, &j_ref);
    let mut new_qualifiers = Vec::with_capacity(comprehension.qualifiers.len() + 1);
    for (i, qualifier) in comprehension.qualifiers.into_iter().enumerate() {
        if i == index {
            new_qualifiers.push(ComprehensionQualifier::ExpressionGenerator { ptr: i_ptr.clone() });
            new_qualifiers.push(ComprehensionQualifier::ExpressionGenerator { ptr: j_ptr.clone() });
        } else {
            new_qualifiers
                .push(qualifier.transform_bi(&|e: Expr| replace_reference(e, &old_ptr, &j_ref)));
        }
    }
    comprehension.qualifiers = new_qualifiers;

    Ok(RuleEffect::pure(Expr::Comprehension(
        metadata.clone(),
        Moo::new(comprehension),
    )))
}

/// `j <- party(wanted, p)` ⟺ `i <- parts(p), wanted in i, j <- i`: find the part containing
/// `wanted`, then iterate its elements. `party` only makes sense when iterated, matching
/// Conjure's own restriction (`rule_Party` only matches inside a comprehension generator).
#[register_rule("Base", 8700, [Comprehension])]
fn expand_party_comprehension_generator(expr: &Expr, _: &SymbolTable) -> ApplicationResult {
    let Expr::Comprehension(metadata, comprehension) = expr else {
        return Err(RuleNotApplicable);
    };

    let Some((index, old_ptr, wanted, partition)) = comprehension
        .qualifiers
        .iter()
        .enumerate()
        .find_map(|(index, qualifier)| {
            let ComprehensionQualifier::ExpressionGenerator { ptr } = qualifier else {
                return None;
            };
            let source = (*ptr.as_quantified_expr()?).clone();
            let Expr::Party(_, wanted, partition) = source else {
                return None;
            };
            Some((index, ptr.clone(), (*wanted).clone(), (*partition).clone()))
        })
    else {
        return Err(RuleNotApplicable);
    };

    let mut comprehension = comprehension.as_ref().clone();
    comprehension.symbols = comprehension.symbols.detach();

    let parts_expr = Expr::Parts(Metadata::new(), Moo::new(partition));
    let i_ptr = DeclarationPtr::new_quantified_expr(Name::user("i"), parts_expr);
    comprehension.symbols.write().update_insert(i_ptr.clone());
    let i_ref = Expr::from(Reference::new(i_ptr.clone()));

    let membership = Expr::In(Metadata::new(), Moo::new(wanted), Moo::new(i_ref.clone()));

    let j_ptr = DeclarationPtr::new_quantified_expr(old_ptr.name().clone(), i_ref);
    comprehension.symbols.write().update_insert(j_ptr.clone());
    let j_ref = Expr::from(Reference::new(j_ptr.clone()));

    comprehension.return_expression =
        replace_reference(comprehension.return_expression, &old_ptr, &j_ref);
    let mut new_qualifiers = Vec::with_capacity(comprehension.qualifiers.len() + 2);
    for (i, qualifier) in comprehension.qualifiers.into_iter().enumerate() {
        if i == index {
            new_qualifiers.push(ComprehensionQualifier::ExpressionGenerator { ptr: i_ptr.clone() });
            new_qualifiers.push(ComprehensionQualifier::Condition(membership.clone()));
            new_qualifiers.push(ComprehensionQualifier::ExpressionGenerator { ptr: j_ptr.clone() });
        } else {
            new_qualifiers
                .push(qualifier.transform_bi(&|e: Expr| replace_reference(e, &old_ptr, &j_ref)));
        }
    }
    comprehension.qualifiers = new_qualifiers;

    Ok(RuleEffect::pure(Expr::Comprehension(
        metadata.clone(),
        Moo::new(comprehension),
    )))
}

fn replace_reference(expr: Expr, old_ptr: &DeclarationPtr, replacement: &Expr) -> Expr {
    expr.transform(&|candidate| match candidate {
        Expr::Atomic(_, Atom::Reference(reference)) if reference.ptr() == old_ptr => {
            replacement.clone()
        }
        other => other,
    })
}

#[cfg(test)]
mod tests {
    use super::*;
    use conjure_cp::ast::{
        AbstractLiteral, Domain, DomainPtr, Literal, Name, PartitionAttr, Range,
    };
    use conjure_cp::{domain_int, range};

    fn partition_ref(name: &str) -> Expr {
        let attr = PartitionAttr {
            num_parts: Range::Single(2),
            part_len: Range::Single(3),
            is_regular: true,
        };
        let dom: DomainPtr = Domain::partition(attr, domain_int!(1..6));
        let decl = DeclarationPtr::new_find(Name::user(name), dom);
        Expr::from(Reference::new(decl))
    }

    #[test]
    fn ground_partition_domain_reports_partition_return_type() {
        assert!(is_partition(&partition_ref("x")));
    }

    #[test]
    fn partition_in_lifts_membership_onto_parts() {
        let lit = Expr::Atomic(
            Metadata::new(),
            Atom::Literal(Literal::AbstractLiteral(AbstractLiteral::Set(vec![
                Literal::Int(1),
                Literal::Int(2),
                Literal::Int(3),
            ]))),
        );
        let in_expr = Expr::In(Metadata::new(), Moo::new(lit), Moo::new(partition_ref("x")));
        let result =
            partition_in(&in_expr, &SymbolTable::new()).expect("should lift membership onto parts");
        assert!(matches!(
            result.new_expression,
            Expr::In(_, _, parts) if matches!(*parts, Expr::Parts(_, _))
        ));
    }

    #[test]
    fn partition_eq_lifts_equality_onto_parts() {
        let eq_expr = Expr::Eq(
            Metadata::new(),
            Moo::new(partition_ref("x")),
            Moo::new(partition_ref("y")),
        );
        let result =
            partition_eq(&eq_expr, &SymbolTable::new()).expect("should lift equality onto parts");
        assert!(matches!(
            result.new_expression,
            Expr::Eq(_, a, b) if matches!(*a, Expr::Parts(_, _)) && matches!(*b, Expr::Parts(_, _))
        ));
    }

    #[test]
    fn partition_card_lifts_onto_participants() {
        let card_expr = Expr::Card(Metadata::new(), Moo::new(partition_ref("x")));
        let result = partition_card(&card_expr, &SymbolTable::new())
            .expect("should lift cardinality onto participants");
        assert!(matches!(
            result.new_expression,
            Expr::Card(_, p) if matches!(*p, Expr::Participants(_, _))
        ));
    }

    #[test]
    fn non_partition_eq_is_not_applicable() {
        let x = Expr::Atomic(Metadata::new(), Atom::Literal(Literal::Int(1)));
        let y = Expr::Atomic(Metadata::new(), Atom::Literal(Literal::Int(2)));
        let eq_expr = Expr::Eq(Metadata::new(), Moo::new(x), Moo::new(y));
        assert!(matches!(
            partition_eq(&eq_expr, &SymbolTable::new()),
            Err(RuleNotApplicable)
        ));
    }
}
