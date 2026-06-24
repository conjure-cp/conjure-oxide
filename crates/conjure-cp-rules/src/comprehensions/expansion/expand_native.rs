use conjure_cp::{
    ast::{
        Atom, DeclarationKind, DeclarationPtr, DomainPtr, Expression, Literal, Metadata, Name,
        SymbolTable,
        ac_operators::ACOperatorKind,
        comprehension::{Comprehension, ComprehensionQualifier},
        eval_constant,
    },
    bug, into_matrix_expr,
    solver::SolverError,
};
use uniplate::Biplate as _;

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
    expand_qualifiers(
        &comprehension,
        0,
        parent_symbols,
        comprehension.skip_operator,
    )
}

fn expand_qualifiers(
    comprehension: &Comprehension,
    qualifier_index: usize,
    parent_symbols: &mut SymbolTable,
    ac_operator: Option<ACOperatorKind>,
) -> Result<Vec<Expression>, SolverError> {
    if qualifier_index == comprehension.qualifiers.len() {
        let child_symbols = comprehension.symbols().clone();
        let return_expression =
            concretise_resolved_reference_atoms(comprehension.return_expression.clone());
        let Some(return_expression) = strip_guarded_safe_index_conditions(return_expression) else {
            return Ok(vec![]);
        };
        let return_expression = simplify_expression(return_expression);
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
            )?,
            Some(false) => vec![],
            None => {
                let suffix = expand_qualifiers(
                    comprehension,
                    qualifier_index + 1,
                    parent_symbols,
                    ac_operator,
                )?;
                apply_guard_to_suffix(condition, suffix, ac_operator)?
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

fn apply_guard_to_suffix(
    guard: &Expression,
    suffix: Vec<Expression>,
    ac_operator: Option<ACOperatorKind>,
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

    Ok(vec![ac_operator.make_skip_operation(guard, suffix)])
}

fn resolve_generator_values(name: &Name, domain: &DomainPtr) -> Result<Vec<Literal>, SolverError> {
    let resolved = domain.resolve().ok_or_else(|| {
        SolverError::ModelFeatureNotSupported(format!(
            "quantified variable '{name}' has unresolved domain after assigning previous generators: {domain}"
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
        DeclarationPtr, Domain, Moo, Range, SymbolTablePtr, comprehension::ComprehensionBuilder,
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
}
