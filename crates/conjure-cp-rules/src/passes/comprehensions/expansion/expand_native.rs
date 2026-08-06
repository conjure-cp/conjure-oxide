use conjure_cp::{
    ast::{
        Atom, DeclarationKind, DeclarationPtr, DomainPtr, Expression, GroundDomain, Literal,
        Metadata, Name, Range, SymbolTable,
        ac_operators::ACOperatorKind,
        comprehension::{Comprehension, ComprehensionQualifier},
        eval_constant,
    },
    bug, into_matrix_expr,
    solver::SolverError,
};
use uniplate::Uniplate as _;

use super::via_solver_common::{
    lift_machine_references_into_parent_scope, simplify_expression,
    strip_guarded_safe_index_conditions,
};

/// Expands the comprehension without calling an external solver.
///
/// Qualifiers are interpreted left-to-right. Generators behave like nested loops, and
/// conditions behave like `if` statements at their position in that loop nest. Constant
/// conditions prune immediately; symbolic conditions in an AC context wrap the expansion
/// of the remaining qualifiers using [`Comprehension::skip_operator`]'s skip semantics.
pub fn expand_native(
    comprehension: Comprehension,
    parent_symbols: &mut SymbolTable,
) -> Result<Vec<Expression>, SolverError> {
    // For Min/Max, the safe value to substitute for a guarded-out element must come from the
    // comprehension's *general* return-expression domain, computed once here before any
    // generator's `with_temporary_quantified_binding` narrows a quantified variable's domain down
    // to one concrete value per branch -- computing it later, from an already-narrowed branch's
    // own (possibly single-valued) result, would make the skip value equal to that one value
    // instead of a bound safe for every possible element, corrupting the guard.
    let min_max_skip_value = match comprehension.skip_operator {
        Some(op @ (ACOperatorKind::Min | ACOperatorKind::Max)) => Some(min_max_skip_value(
            &comprehension.return_expression,
            op == ACOperatorKind::Min,
        )?),
        _ => None,
    };
    expand_qualifiers(
        &comprehension,
        0,
        parent_symbols,
        comprehension.skip_operator,
        min_max_skip_value,
    )
}

/// The domain bound to substitute for a guarded-out element in a min/max skip operation: the
/// return expression's own upper bound (for `min`) or lower bound (for `max`). Minion requires
/// every decision variable to have a finite, bounded domain, so this is always expected to
/// resolve; if it somehow doesn't, that's a genuine "can't expand this comprehension" failure
/// (not a bug to hide), surfaced as a model-feature error rather than a panic.
fn min_max_skip_value(
    return_expression: &Expression,
    want_max: bool,
) -> Result<Literal, SolverError> {
    let not_supported = || {
        SolverError::ModelFeatureNotSupported(format!(
            "min/max comprehension with a symbolic guard: could not determine a bounded domain \
             for the return expression {return_expression} to build a safe skip value"
        ))
    };
    let ranges = return_expression
        .domain_of()
        .and_then(|domain| domain.as_ground().cloned())
        .and_then(|ground| match ground {
            GroundDomain::Int(ranges) => Some(ranges),
            _ => None,
        })
        .ok_or_else(not_supported)?;
    let spanning = Range::spanning(&ranges);
    let bound = if want_max {
        spanning.high()
    } else {
        spanning.low()
    };
    Ok(Literal::Int(*bound.ok_or_else(not_supported)?))
}

fn expand_qualifiers(
    comprehension: &Comprehension,
    qualifier_index: usize,
    parent_symbols: &mut SymbolTable,
    ac_operator: Option<ACOperatorKind>,
    min_max_skip_value: Option<Literal>,
) -> Result<Vec<Expression>, SolverError> {
    if qualifier_index == comprehension.qualifiers.len() {
        let child_symbols = comprehension.symbols().clone();
        let return_expression =
            concretise_resolved_reference_atoms(comprehension.return_expression.clone());
        // Comprehensions are leaves in Expression's Uniplate traversal. Expand any nested
        // comprehensions while the current generators still have their temporary bindings, so
        // dependent domains in the nested comprehension can resolve those bindings.
        let return_expression = expand_nested_comprehensions(return_expression, parent_symbols)?;
        let Some(return_expression) = strip_guarded_safe_index_conditions(return_expression) else {
            return Ok(vec![]);
        };
        let return_expression = simplify_expression(return_expression);
        // Drop AC identities here so And/Or/Sum/Product expansions do not materialise a huge
        // vector of tautologies that the rewriter must later strip. Min/Max have no universal
        // identity (ACOperatorKind::identity() panics for them), so this optimisation just
        // doesn't apply to them -- every element stays, which is still correct, just not maximally
        // compact.
        if let Some(op) = ac_operator
            && !matches!(op, ACOperatorKind::Min | ACOperatorKind::Max)
            && let Expression::Atomic(_, Atom::Literal(lit)) = &return_expression
            && lit == &op.identity()
        {
            return Ok(vec![]);
        }
        let return_expression = lift_machine_references_into_parent_scope(
            return_expression,
            &child_symbols,
            parent_symbols,
        );
        return Ok(vec![return_expression]);
    }

    let expanded = match &comprehension.qualifiers[qualifier_index] {
        ComprehensionQualifier::Generator { ptr } => {
            let name = ptr.name().clone();
            let domain = ptr.domain().expect("generator declaration has domain");
            let values = resolve_generator_values(&name, &domain)?;
            let mut expanded = Vec::new();

            for literal in values {
                let mut suffix = with_temporary_quantified_binding(ptr, &literal, || {
                    expand_qualifiers(
                        comprehension,
                        qualifier_index + 1,
                        parent_symbols,
                        ac_operator,
                        min_max_skip_value.clone(),
                    )
                })?;
                expanded.append(&mut suffix);
            }

            expanded
        }
        ComprehensionQualifier::Condition(condition) => match evaluate_bool_guard(condition)? {
            Some(true) => expand_qualifiers(
                comprehension,
                qualifier_index + 1,
                parent_symbols,
                ac_operator,
                min_max_skip_value,
            )?,
            Some(false) => vec![],
            None => {
                let suffix = expand_qualifiers(
                    comprehension,
                    qualifier_index + 1,
                    parent_symbols,
                    ac_operator,
                    min_max_skip_value.clone(),
                )?;
                apply_guard_to_suffix(condition, suffix, ac_operator, min_max_skip_value)?
            }
        },
        ComprehensionQualifier::ExpressionGenerator { .. } => {
            // See `expand_comprehension_native`: expression generators are not unrolled natively.
            bug!(
                "Comprehension expander should not be called on comprehensions containing ExpressionGenerator"
            );
        }
    };

    Ok(expanded)
}

fn expand_nested_comprehensions(
    expr: Expression,
    parent_symbols: &mut SymbolTable,
) -> Result<Expression, SolverError> {
    let children = expr
        .children()
        .into_iter()
        .map(|child| expand_nested_comprehensions(child, parent_symbols))
        .collect::<Result<_, _>>()?;
    let expr = expr.with_children(children);

    let Expression::Comprehension(_, comprehension) = expr else {
        return Ok(expr);
    };

    let results = expand_native(comprehension.as_ref().clone(), parent_symbols)?;
    Ok(into_matrix_expr!(results))
}

fn apply_guard_to_suffix(
    guard: &Expression,
    suffix: Vec<Expression>,
    ac_operator: Option<ACOperatorKind>,
    min_max_skip_value: Option<Literal>,
) -> Result<Vec<Expression>, SolverError> {
    if suffix.is_empty() {
        return Ok(vec![]);
    }

    let Some(ac_operator) = ac_operator else {
        return Err(SolverError::ModelInvalid(format!(
            "comprehension has symbolic guard but no AC operator context for native expansion: {guard:?}"
        )));
    };

    let guard = concretise_resolved_reference_atoms(guard.clone());
    let guard = simplify_expression(guard);
    let suffix = ac_operator.as_expression(into_matrix_expr!(suffix));

    let skip_expr = match (ac_operator, min_max_skip_value) {
        (ACOperatorKind::Min | ACOperatorKind::Max, Some(skip_value)) => {
            ac_operator.make_min_max_skip_operation(guard, suffix, skip_value)
        }
        (ACOperatorKind::Min | ACOperatorKind::Max, None) => {
            bug!("min/max comprehension expansion is missing its precomputed skip value")
        }
        _ => ac_operator.make_skip_operation(guard, suffix),
    };
    Ok(vec![skip_expr])
}

fn resolve_generator_values(name: &Name, domain: &DomainPtr) -> Result<Vec<Literal>, SolverError> {
    let resolved = domain.resolve().map_err(|e| {
        SolverError::ModelFeatureNotSupported(format!(
            "quantified variable '{name}' has unresolved domain after assigning previous generators: {domain}; error: {e}"
        ))
    })?;

    resolved.values().map(|iter| iter.collect()).map_err(|err| {
        SolverError::ModelFeatureNotSupported(format!(
            "quantified variable '{name}' has non-enumerable domain: {err}"
        ))
    })
}

fn with_temporary_quantified_binding<T>(
    quantified: &DeclarationPtr,
    value: &Literal,
    f: impl FnOnce() -> Result<T, SolverError>,
) -> Result<T, SolverError> {
    let mut targets = vec![quantified.clone()];
    if let DeclarationKind::Quantified(inner) = &*quantified.kind()
        && let Some(generator) = inner.generator()
    {
        targets.push(generator.clone());
    }

    let mut originals = Vec::with_capacity(targets.len());
    for mut target in targets {
        let old_kind = target.replace_kind(DeclarationKind::TemporaryValueLetting(
            Expression::Atomic(Metadata::new(), Atom::Literal(value.clone())),
        ));
        originals.push((target, old_kind));
    }

    let result = f();

    for (mut target, old_kind) in originals.into_iter().rev() {
        let _ = target.replace_kind(old_kind);
    }

    result
}

/// Returns `Ok(Some(bool))` for constant guards, `Ok(None)` for symbolic guards.
fn evaluate_bool_guard(guard: &Expression) -> Result<Option<bool>, SolverError> {
    let simplified = simplify_expression(guard.clone());
    match eval_constant(&simplified) {
        Some(Literal::Bool(value)) => Ok(Some(value)),
        Some(other) => Err(SolverError::ModelInvalid(format!(
            "native comprehension guard must evaluate to Bool, got {other}: {guard}"
        ))),
        None => Ok(None),
    }
}

fn concretise_resolved_reference_atoms(expr: Expression) -> Expression {
    use uniplate::Biplate as _;
    expr.transform_bi(&|atom: Atom| match atom {
        Atom::Reference(reference) => reference
            .resolve_constant()
            .map_or_else(|| Atom::Reference(reference), Atom::Literal),
        other => other,
    })
}

#[cfg(test)]
mod tests {
    use conjure_cp::ast::{
        DeclarationPtr, Domain, IntVal, Moo, Range, Reference, SymbolTablePtr,
        comprehension::ComprehensionBuilder,
    };

    use super::*;

    fn atom_ref(ptr: DeclarationPtr) -> Expression {
        Expression::Atomic(Metadata::new(), Atom::new_ref(ptr))
    }

    fn int(value: i32) -> Expression {
        Expression::Atomic(Metadata::new(), Atom::Literal(Literal::Int(value)))
    }

    #[test]
    fn constant_guard_prunes_false_branches_without_identity_elements() {
        let parent_symbols = SymbolTablePtr::new();
        let mut builder = ComprehensionBuilder::new(parent_symbols.clone());
        builder = builder.generator(DeclarationPtr::new_find(
            Name::user("i"),
            Domain::int(vec![Range::Bounded(1, 9)]),
        ));
        let i = builder
            .generator_symboltable()
            .read()
            .lookup_local(&Name::user("i"))
            .expect("i should be in comprehension scope");

        let i_expr = atom_ref(i);
        builder = builder.guard(Expression::Eq(
            Metadata::new(),
            Moo::new(Expression::UnsafeMod(
                Metadata::new(),
                Moo::new(i_expr.clone()),
                Moo::new(int(2)),
            )),
            Moo::new(int(0)),
        ));

        let comprehension = builder.with_return_value(i_expr);
        let expanded = expand_native(comprehension, &mut parent_symbols.read().clone()).unwrap();

        assert_eq!(expanded, vec![int(2), int(4), int(6), int(8)]);
    }

    #[test]
    fn min_max_skip_value_uses_the_return_expressions_domain_bound() {
        let i = DeclarationPtr::new_find(Name::user("i"), Domain::int(vec![Range::Bounded(5, 8)]));
        let i_expr = atom_ref(i);

        assert_eq!(
            min_max_skip_value(&i_expr, true).unwrap(),
            Literal::Int(8),
            "min's skip value should be the upper bound"
        );
        assert_eq!(
            min_max_skip_value(&i_expr, false).unwrap(),
            Literal::Int(5),
            "max's skip value should be the lower bound"
        );
    }

    #[test]
    fn min_comprehension_with_a_symbolic_guard_never_lets_the_skip_value_win() {
        // Regression test: min/max comprehensions used to be tagged with ACOperatorKind::Sum as
        // their skip_operator, so a symbolic guard substituted 0 (Sum's identity) for guarded-out
        // elements -- not a neutral value for min. Domain is entirely positive (6..8) so 0 would
        // have won the min if the bug were still present.
        let parent_symbols = SymbolTablePtr::new();
        let b = DeclarationPtr::new_find(Name::user("b"), Domain::bool());
        parent_symbols.write().insert(b.clone());

        let mut builder = ComprehensionBuilder::new(parent_symbols.clone()).generator(
            DeclarationPtr::new_find(Name::user("i"), Domain::int(vec![Range::Bounded(6, 8)])),
        );
        let i = builder
            .generator_symboltable()
            .read()
            .lookup_local(&Name::user("i"))
            .expect("i should be in comprehension scope");
        let i_expr = atom_ref(i);

        // `i != 6 \/ b`: with b left symbolic, only i=6's guard is undetermined at expansion time.
        builder = builder.guard(Expression::Or(
            Metadata::new(),
            Moo::new(conjure_cp::matrix_expr![
                Expression::Neq(Metadata::new(), Moo::new(i_expr.clone()), Moo::new(int(6))),
                atom_ref(b),
            ]),
        ));

        let mut comprehension = builder.with_return_value(i_expr);
        comprehension.skip_operator = Some(ACOperatorKind::Min);
        let expanded = expand_native(comprehension, &mut parent_symbols.read().clone()).unwrap();

        // Every literal `0` in the expanded tree would indicate the old Sum-identity bug; the
        // skip value must be the domain's own upper bound (8) instead.
        let flat = format!("{expanded:?}");
        assert!(
            !flat.contains("Int(0)"),
            "min skip value must not be Sum's identity (0): {flat}"
        );
        assert!(
            flat.contains("Int(8)"),
            "min skip value should be the domain upper bound (8): {flat}"
        );
    }

    #[test]
    fn native_expansion_drops_and_identity_results() {
        let parent_symbols = SymbolTablePtr::new();
        let mut comprehension = ComprehensionBuilder::new(parent_symbols.clone())
            .generator(DeclarationPtr::new_find(
                Name::user("i"),
                Domain::int(vec![Range::Bounded(1, 3)]),
            ))
            .with_return_value(Expression::from(true));
        comprehension.skip_operator = Some(ACOperatorKind::And);

        let expanded = expand_native(comprehension, &mut parent_symbols.read().clone()).unwrap();
        assert!(expanded.is_empty());
    }

    #[test]
    fn nested_comprehension_domains_see_outer_generator_bindings() {
        let parent_symbols = SymbolTablePtr::new();
        let mut outer = ComprehensionBuilder::new(parent_symbols.clone()).generator(
            DeclarationPtr::new_find(Name::user("i"), Domain::int(vec![Range::Bounded(1, 2)])),
        );
        let i = outer
            .generator_symboltable()
            .read()
            .lookup_local(&Name::user("i"))
            .expect("i should be in outer comprehension scope");

        let mut inner = ComprehensionBuilder::new(outer.generator_symboltable()).generator(
            DeclarationPtr::new_find(
                Name::user("j"),
                Domain::int(vec![Range::Single(IntVal::Reference(Reference::new(i)))]),
            ),
        );
        let j = inner
            .generator_symboltable()
            .read()
            .lookup_local(&Name::user("j"))
            .expect("j should be in inner comprehension scope");
        let inner = Expression::Comprehension(
            Metadata::new(),
            Moo::new(inner.with_return_value(atom_ref(j))),
        );
        let outer = outer.with_return_value(inner);

        let expanded = expand_native(outer, &mut parent_symbols.read().clone()).unwrap();

        assert_eq!(
            expanded,
            vec![
                simplify_expression(into_matrix_expr!(vec![int(1)])),
                simplify_expression(into_matrix_expr!(vec![int(2)])),
            ]
        );
    }
}
