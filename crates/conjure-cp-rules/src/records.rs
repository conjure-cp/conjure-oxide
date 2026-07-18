use crate::guard;
use crate::representation::record_to_tuple::RecordToTuple;
use conjure_cp::ast::{
    AbstractLiteral, Atom, Expression, Literal, Metadata, Reference, SymbolTable,
};
use conjure_cp::bug::UnwrapOrBug;
use conjure_cp::rule_engine::{
    ApplicationError::RuleNotApplicable, ApplicationResult, RuleEffect as Reduction, register_rule,
};
use uniplate::Uniplate;

/// Indexing into a record variable
/// e.g:
/// ```plain
/// x[a]
/// ~>
/// x_RecordToTuple[1]
/// ```
/// where
/// ```plain
/// x: record { a : bool, b : int(0..9) }
/// ```
#[register_rule("ReprGeneral", 9500, [RecordField])]
fn index_record_to_tuple(expr: &Expression, _: &SymbolTable) -> ApplicationResult {
    guard!(
        let Expression::RecordField(_, rec_expr, field_name) = expr        &&
        let Expression::Atomic(_, Atom::Reference(re)) = rec_expr.as_ref() &&
        let Some(repr) = re.get_repr_as::<RecordToTuple>()
        else {
            return Err(RuleNotApplicable);
        }
    );

    let new_expr = repr.name_to_idx_expr(field_name).unwrap_or_bug();
    Ok(Reduction::pure(new_expr))
}

/// Convert all record literals to tuples
#[register_rule("ReprGeneral", 9600, [Atomic / Literal])]
fn record_lit_to_tuple(expr: &Expression, _: &SymbolTable) -> ApplicationResult {
    guard!(
        let Expression::Atomic(_, Atom::Literal(lit)) = expr              &&
        let Literal::AbstractLiteral(AbstractLiteral::Record(ents)) = lit
        else {
            return Err(RuleNotApplicable);
        }
    );

    let mut ents = ents.clone();
    ents.sort();

    let tuple = AbstractLiteral::Tuple(ents.into_iter().map(|x| x.value).collect());
    let new_expr = Expression::Atomic(Metadata::new(), Atom::Literal(tuple.into()));
    Ok(Reduction::pure(new_expr))
}

/// Convert all record expressions to tuples
#[register_rule("ReprGeneral", 9600, [AbstractLiteral])]
fn record_abslit_to_tuple(expr: &Expression, _: &SymbolTable) -> ApplicationResult {
    let Expression::AbstractLiteral(_, AbstractLiteral::Record(ents)) = expr else {
        return Err(RuleNotApplicable);
    };

    let mut ents = ents.clone();
    ents.sort();

    let tuple = AbstractLiteral::Tuple(ents.into_iter().map(|x| x.value).collect());
    let new_expr = Expression::AbstractLiteral(Metadata::new(), tuple);
    Ok(Reduction::pure(new_expr))
}

/// Convert record references (and record literals) that appear as children of value-level
/// containers into their tuple representation.
///
/// The prefilter is intentionally narrow: record *values* appear under equality,
/// disequality, and bubbles. Nested record literals inside other abstract literals are
/// lowered when those literals are themselves focused by [`record_lit_to_tuple`] /
/// [`record_abslit_to_tuple`]. A universal `* / Atomic` (or even `AbstractLiteral / Atomic`)
/// child prefilter was attempted on nearly every parent of an integer/bool atom — including
/// large post-expansion matrix literals — dominating rule-attempt volume on models that
/// never use records.
///
/// Indexing parents (`SafeIndex` / `UnsafeIndex` / `RecordField`) are excluded so field
/// access can still see the record reference and lower via [`index_record_to_tuple`].
#[register_rule(
    "ReprGeneral",
    9700,
    [
        Eq / Atomic,
        Eq / AbstractLiteral,
        Neq / Atomic,
        Neq / AbstractLiteral,
        Bubble / Atomic,
        Bubble / AbstractLiteral
    ]
)]
fn ref_record_to_tuple(expr: &Expression, _: &SymbolTable) -> ApplicationResult {
    if let Expression::SafeIndex(..) | Expression::UnsafeIndex(..) | Expression::RecordField(..) =
        expr
    {
        return Err(RuleNotApplicable);
    };

    let mut changed = false;
    let new_children = expr
        .children()
        .into_iter()
        .map(|expr| {
            if let Expression::Atomic(_, Atom::Reference(re)) = &expr
                && let Some(repr) = re.get_repr_as::<RecordToTuple>()
            {
                changed = true;
                Reference::new(repr.tuple.clone()).into()
            } else if let Expression::Atomic(
                _,
                Atom::Literal(Literal::AbstractLiteral(AbstractLiteral::Record(ents))),
            ) = &expr
            {
                changed = true;
                let mut ents = ents.clone();
                ents.sort();
                let tuple = AbstractLiteral::Tuple(ents.into_iter().map(|x| x.value).collect());
                Expression::Atomic(Metadata::new(), Atom::Literal(tuple.into()))
            } else if let Expression::AbstractLiteral(_, AbstractLiteral::Record(ents)) = &expr {
                changed = true;
                let mut ents = ents.clone();
                ents.sort();
                let tuple = AbstractLiteral::Tuple(ents.into_iter().map(|x| x.value).collect());
                Expression::AbstractLiteral(Metadata::new(), tuple)
            } else {
                expr
            }
        })
        .collect();

    if changed {
        Ok(Reduction::pure(expr.with_children(new_children)))
    } else {
        Err(RuleNotApplicable)
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use conjure_cp::ast::records::Field;
    use conjure_cp::ast::{DeclarationPtr, Domain, Moo, Name, Range};
    use conjure_cp::into_matrix_expr;
    use conjure_cp::representation::ReprRule;
    use conjure_cp::rule_engine::{ApplicationError, get_rule_by_name};

    /// Builds `and([x, x, ...])` over an int find — a common non-record parent shape.
    fn and_of_int_refs(n: usize) -> (SymbolTable, Expression) {
        let mut symbols = SymbolTable::new();
        let decl =
            DeclarationPtr::new_find(Name::user("x"), Domain::int(vec![Range::Bounded(1, 4)]));
        symbols
            .insert(decl.clone())
            .expect("int find should insert");
        let refs: Vec<Expression> = (0..n)
            .map(|_| {
                Expression::Atomic(
                    Metadata::new(),
                    Atom::Reference(Reference::new(decl.clone())),
                )
            })
            .collect();
        let and_expr = Expression::And(Metadata::new(), Moo::new(into_matrix_expr!(refs)));
        (symbols, and_expr)
    }

    #[test]
    fn ref_record_to_tuple_rejects_non_record_and_parent() {
        let (symbols, and_expr) = and_of_int_refs(32);
        let rule = get_rule_by_name("ref_record_to_tuple").expect("rule registered");
        let err = rule.apply(&and_expr, &symbols).unwrap_err();
        assert!(matches!(err, ApplicationError::RuleNotApplicable));
    }

    #[test]
    fn ref_record_to_tuple_prefilter_targets_eq_not_universal_atomic_child() {
        use conjure_cp::rule_engine::RulePrefilter;

        let rule = get_rule_by_name("ref_record_to_tuple").expect("rule registered");
        let prefilters = rule.prefilters.expect("prefilter must be present");
        assert!(
            prefilters
                .iter()
                .any(|p| matches!(p, RulePrefilter::VariantChild { .. })),
            "expected VariantChild alternatives such as Eq / Atomic"
        );
        assert!(
            !prefilters
                .iter()
                .any(|p| matches!(p, RulePrefilter::Child { .. })),
            "universal * / Atomic child prefilters must not remain"
        );
    }

    #[test]
    fn ref_record_to_tuple_rewrites_eq_of_record_refs() {
        let mut symbols = SymbolTable::new();
        let domain = Domain::record(vec![
            Field {
                name: Name::user("a"),
                value: Domain::bool(),
            },
            Field {
                name: Name::user("b"),
                value: Domain::int(vec![Range::Bounded(0, 9)]),
            },
        ]);
        let y = DeclarationPtr::new_find(Name::user("y"), domain.clone());
        let z = DeclarationPtr::new_find(Name::user("z"), domain);
        symbols.insert(y.clone()).expect("y");
        symbols.insert(z.clone()).expect("z");

        let mut y_init = y.clone();
        let (extra_y, _) = RecordToTuple::init_for(&mut y_init).unwrap();
        symbols.update_insert(y_init.clone());
        symbols.extend(extra_y);

        let mut z_init = z.clone();
        let (extra_z, _) = RecordToTuple::init_for(&mut z_init).unwrap();
        symbols.update_insert(z_init.clone());
        symbols.extend(extra_z);

        // Selection lives on the Reference, not only on the declaration store.
        let mut y_ref = Reference::new(y_init);
        let _ = y_ref
            .select_repr::<RecordToTuple>()
            .expect("y RecordToTuple selectable");
        let mut z_ref = Reference::new(z_init);
        let _ = z_ref
            .select_repr::<RecordToTuple>()
            .expect("z RecordToTuple selectable");

        let eq = Expression::Eq(
            Metadata::new(),
            Moo::new(Expression::Atomic(Metadata::new(), Atom::Reference(y_ref))),
            Moo::new(Expression::Atomic(Metadata::new(), Atom::Reference(z_ref))),
        );

        let rule = get_rule_by_name("ref_record_to_tuple").expect("rule registered");
        let result = rule.apply(&eq, &symbols).expect("should rewrite Eq");
        let Expression::Eq(_, lhs, rhs) = result.new_expression else {
            panic!("expected Eq result");
        };
        assert!(matches!(
            lhs.as_ref(),
            Expression::Atomic(_, Atom::Reference(_))
        ));
        assert!(matches!(
            rhs.as_ref(),
            Expression::Atomic(_, Atom::Reference(_))
        ));
        // Rewritten references must not still carry RecordToTuple (they point at the tuple).
        if let Expression::Atomic(_, Atom::Reference(re)) = lhs.as_ref() {
            assert!(re.get_repr_as::<RecordToTuple>().is_none());
        }
    }
}
