use conjure_cp::{
    ast::{
        Atom, DeclarationKind, DeclarationPtr, DomainPtr, Expression, Literal, Metadata, Name,
        SymbolTable,
        comprehension::{Comprehension, ComprehensionQualifier},
        eval_constant,
    },
    solver::SolverError,
};
use uniplate::Biplate as _;

use super::via_solver_common::{lift_machine_references_into_parent_scope, simplify_expression};

/// Expands the comprehension without calling an external solver.
///
/// Algorithm:
/// 1. Recurse qualifiers left-to-right.
/// 2. For each generator value, temporarily bind the quantified declaration to a
///    `TemporaryValueLetting` and recurse.
/// 3. For each condition, evaluate and recurse only if true.
/// 4. At the leaf, evaluate the return expression under the active bindings.
pub fn expand_native(
    comprehension: Comprehension,
    parent_symbols: &mut SymbolTable,
) -> Result<Vec<Expression>, SolverError> {
    let mut expanded = Vec::new();
    expand_qualifiers(&comprehension, 0, &mut expanded, parent_symbols)?;
    Ok(expanded)
}

fn expand_qualifiers(
    comprehension: &Comprehension,
    qualifier_index: usize,
    expanded: &mut Vec<Expression>,
    parent_symbols: &mut SymbolTable,
) -> Result<(), SolverError> {
    if qualifier_index == comprehension.qualifiers.len() {
        let child_symbols = comprehension.symbols().clone();
        let return_expression =
            concretise_resolved_reference_atoms(comprehension.return_expression.clone());
        let return_expression = simplify_expression(return_expression);
        let return_expression = lift_machine_references_into_parent_scope(
            return_expression,
            &child_symbols,
            parent_symbols,
        );
        expanded.push(return_expression);
        return Ok(());
    }

    match &comprehension.qualifiers[qualifier_index] {
        ComprehensionQualifier::Generator { name, domain } => {
            let values = resolve_generator_values(name, domain)?;
            let quantified_declaration = lookup_quantified_declaration(comprehension, name)?;

            for literal in values {
                with_temporary_quantified_binding(&quantified_declaration, &literal, || {
                    expand_qualifiers(comprehension, qualifier_index + 1, expanded, parent_symbols)
                })?;
            }
        }
        ComprehensionQualifier::Condition(condition) => {
            if evaluate_guard(condition)? {
                expand_qualifiers(comprehension, qualifier_index + 1, expanded, parent_symbols)?;
            }
        }
    }

    Ok(())
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

fn lookup_quantified_declaration(
    comprehension: &Comprehension,
    name: &Name,
) -> Result<DeclarationPtr, SolverError> {
    comprehension.symbols().lookup_local(name).ok_or_else(|| {
        SolverError::ModelInvalid(format!(
            "quantified variable '{name}' is missing from local comprehension symbol table"
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

fn evaluate_guard(guard: &Expression) -> Result<bool, SolverError> {
    let simplified = simplify_expression(guard.clone());
    match eval_constant(&simplified) {
        Some(Literal::Bool(value)) => Ok(value),
        Some(other) => Err(SolverError::ModelInvalid(format!(
            "native comprehension guard must evaluate to Bool, got {other}: {guard}"
        ))),
        None => Err(SolverError::ModelInvalid(format!(
            "native comprehension expansion could not evaluate guard: {guard}"
        ))),
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
