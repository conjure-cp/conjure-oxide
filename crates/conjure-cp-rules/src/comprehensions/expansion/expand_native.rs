use std::collections::{HashMap, HashSet};

use conjure_cp::{
    ast::{
        Atom, DeclarationKind, DeclarationPtr, Domain, DomainPtr, Expression, GroundDomain, IntVal,
        Literal, Metadata, Moo, Name, Range, SymbolTable, UnresolvedDomain,
        comprehension::{Comprehension, ComprehensionQualifier},
        eval_constant,
        serde::HasId as _,
    },
    solver::SolverError,
};
use uniplate::Biplate as _;

use super::via_solver_common::{lift_machine_references_into_parent_scope, simplify_expression};

/// Expands the comprehension without calling an external solver.
///
/// Quantified variables are enumerated with native Rust loops over their finite domains. Guards
/// are evaluated from the currently bound quantified declarations.
pub fn expand_native(
    comprehension: Comprehension,
    symtab: &mut SymbolTable,
) -> Result<Vec<Expression>, SolverError> {
    let mut assignments = HashMap::new();
    let mut expanded = Vec::new();
    let quantified_names: HashSet<Name> = comprehension
        .qualifiers
        .iter()
        .filter_map(|qualifier| match qualifier {
            ComprehensionQualifier::Generator { name, .. } => Some(name.clone()),
            ComprehensionQualifier::Condition(_) => None,
        })
        .collect();
    let binding_targets = collect_quantified_binding_targets(&comprehension, &quantified_names);

    enumerate_assignments(
        0,
        &comprehension.qualifiers,
        &binding_targets,
        &mut assignments,
        &mut |_assignment| {
            let child_symtab = comprehension.symbols().clone();
            let return_expression = comprehension.return_expression.clone();
            let return_expression = simplify_expression(return_expression);
            let return_expression =
                lift_machine_references_into_parent_scope(return_expression, &child_symtab, symtab);

            expanded.push(return_expression);
            Ok(())
        },
    )?;

    Ok(expanded)
}

fn enumerate_assignments(
    qualifier_index: usize,
    qualifiers: &[ComprehensionQualifier],
    binding_targets: &HashMap<Name, Vec<DeclarationPtr>>,
    assignment: &mut HashMap<Name, Literal>,
    on_assignment: &mut impl FnMut(&HashMap<Name, Literal>) -> Result<(), SolverError>,
) -> Result<(), SolverError> {
    if qualifier_index == qualifiers.len() {
        return on_assignment(assignment);
    }

    match &qualifiers[qualifier_index] {
        ComprehensionQualifier::Generator { name, domain } => {
            let resolved =
                resolve_domain_for_assignment(domain.clone(), assignment).ok_or_else(|| {
                    SolverError::ModelFeatureNotSupported(format!(
                        "quantified variable '{name}' has unresolved domain after assigning previous generators: {domain}"
                    ))
                })?;

            let values: Vec<Literal> = resolved
                .values()
                .map_err(|err| {
                    SolverError::ModelFeatureNotSupported(format!(
                        "quantified variable '{name}' has non-enumerable domain: {err}"
                    ))
                })?
                .collect();

            let targets = binding_targets.get(name).cloned().ok_or_else(|| {
                SolverError::ModelInvalid(format!(
                    "quantified variable '{name}' has no binding targets in comprehension scope"
                ))
            })?;

            for lit in values {
                assignment.insert(name.clone(), lit.clone());
                let _binding = QuantifiedBinding::bind_all(&targets, &lit);

                enumerate_assignments(
                    qualifier_index + 1,
                    qualifiers,
                    binding_targets,
                    assignment,
                    on_assignment,
                )?;

                assignment.remove(name);
            }
        }
        ComprehensionQualifier::Condition(condition) => match evaluate_guard(condition) {
            Some(true) => {
                enumerate_assignments(
                    qualifier_index + 1,
                    qualifiers,
                    binding_targets,
                    assignment,
                    on_assignment,
                )?;
            }
            Some(false) => {}
            None => {
                return Err(SolverError::ModelInvalid(format!(
                    "native comprehension expansion could not evaluate guard: {condition}"
                )));
            }
        },
    }

    Ok(())
}

fn evaluate_guard(guard: &Expression) -> Option<bool> {
    let simplified = simplify_expression(guard.clone());
    match eval_constant(&simplified)? {
        Literal::Bool(value) => Some(value),
        _ => None,
    }
}

fn resolve_domain_for_assignment(
    domain: DomainPtr,
    assignment: &HashMap<Name, Literal>,
) -> Option<Moo<GroundDomain>> {
    if let Some(resolved) = domain.resolve() {
        return Some(resolved);
    }

    let Domain::Unresolved(unresolved) = domain.as_ref() else {
        return None;
    };

    let UnresolvedDomain::Int(ranges) = unresolved.as_ref() else {
        return None;
    };

    let ranges = ranges
        .iter()
        .map(|range| resolve_range_from_bound_declarations(range, assignment))
        .collect::<Option<Vec<_>>>()?;

    Some(Moo::new(GroundDomain::Int(ranges)))
}

fn resolve_range_from_bound_declarations(
    range: &Range<IntVal>,
    assignment: &HashMap<Name, Literal>,
) -> Option<Range<i32>> {
    Some(match range {
        Range::Single(x) => Range::Single(resolve_int_val_from_bound_declarations(x, assignment)?),
        Range::Bounded(l, r) => Range::Bounded(
            resolve_int_val_from_bound_declarations(l, assignment)?,
            resolve_int_val_from_bound_declarations(r, assignment)?,
        ),
        Range::UnboundedL(r) => {
            Range::UnboundedL(resolve_int_val_from_bound_declarations(r, assignment)?)
        }
        Range::UnboundedR(l) => {
            Range::UnboundedR(resolve_int_val_from_bound_declarations(l, assignment)?)
        }
        Range::Unbounded => Range::Unbounded,
    })
}

fn resolve_int_val_from_bound_declarations(
    value: &IntVal,
    assignment: &HashMap<Name, Literal>,
) -> Option<i32> {
    match value {
        IntVal::Const(x) => Some(*x),
        IntVal::Reference(reference) => {
            if let Some(Literal::Int(x)) = reference.resolve_constant() {
                return Some(x);
            }

            let reference_name = reference.name().clone();
            let assigned = assignment.get(&reference_name).cloned().or_else(|| {
                assignment.iter().find_map(|(name, lit)| {
                    if name.to_string() == reference_name.to_string() {
                        Some(lit.clone())
                    } else {
                        None
                    }
                })
            });

            match assigned {
                Some(Literal::Int(x)) => Some(x),
                _ => None,
            }
        }
        IntVal::Expr(expr) => {
            let expr = substitute_assigned_references(expr.as_ref().clone(), assignment);
            let simplified = simplify_expression(expr);
            match eval_constant(&simplified) {
                Some(Literal::Int(x)) => Some(x),
                _ => None,
            }
        }
    }
}

fn substitute_assigned_references(
    expr: Expression,
    assignment: &HashMap<Name, Literal>,
) -> Expression {
    expr.transform_bi(&|atom: Atom| {
        let Atom::Reference(reference) = atom else {
            return atom;
        };

        let reference_name = reference.name().clone();
        let assigned = assignment.get(&reference_name).cloned().or_else(|| {
            assignment.iter().find_map(|(name, lit)| {
                if name.to_string() == reference_name.to_string() {
                    Some(lit.clone())
                } else {
                    None
                }
            })
        });

        match assigned {
            Some(lit) => Atom::Literal(lit),
            None => Atom::Reference(reference),
        }
    })
}

fn collect_quantified_binding_targets(
    comprehension: &Comprehension,
    quantified_names: &HashSet<Name>,
) -> HashMap<Name, Vec<DeclarationPtr>> {
    let mut result: HashMap<Name, Vec<DeclarationPtr>> = HashMap::new();

    let mut add_target = |name: &Name, decl: DeclarationPtr| {
        let entry = result.entry(name.clone()).or_default();
        if !entry.iter().any(|existing| existing.id() == decl.id()) {
            entry.push(decl);
        }
    };

    let symbols = comprehension.symbols().clone();
    for (name, decl) in symbols.into_iter_local() {
        if !quantified_names.contains(&name) {
            continue;
        }

        let generator = {
            let kind = decl.kind();
            if let DeclarationKind::Quantified(inner) = &*kind {
                inner.generator().cloned()
            } else {
                None
            }
        };

        let is_quantified = {
            let kind = decl.kind();
            matches!(&*kind, DeclarationKind::Quantified(_))
        };

        if generator.is_some() || is_quantified {
            add_target(&name, decl.clone());
        }
        if let Some(generator) = generator {
            add_target(&name, generator);
        }
    }

    // Also collect from concrete references in guards and return expression.
    for guard in comprehension.generator_conditions() {
        let refs: std::collections::VecDeque<DeclarationPtr> = guard.universe_bi();
        for decl in refs {
            let name = decl.name().clone();
            if !quantified_names.contains(&name) {
                continue;
            }
            add_target(&name, decl.clone());
            let kind = decl.kind();
            if let DeclarationKind::Quantified(inner) = &*kind
                && let Some(generator) = inner.generator()
            {
                add_target(&name, generator.clone());
            }
        }
    }

    let refs: std::collections::VecDeque<DeclarationPtr> =
        comprehension.return_expression.clone().universe_bi();
    for decl in refs {
        let name = decl.name().clone();
        if !quantified_names.contains(&name) {
            continue;
        }
        add_target(&name, decl.clone());
        let kind = decl.kind();
        if let DeclarationKind::Quantified(inner) = &*kind
            && let Some(generator) = inner.generator()
        {
            add_target(&name, generator.clone());
        }
    }

    result
}

struct QuantifiedBinding {
    targets: Vec<(DeclarationPtr, DeclarationKind)>,
}

impl QuantifiedBinding {
    fn bind_all(targets: &[DeclarationPtr], lit: &Literal) -> Self {
        let mut previous = Vec::with_capacity(targets.len());
        for target in targets {
            let mut mutable_target = target.clone();
            let previous_kind =
                mutable_target.replace_kind(DeclarationKind::TemporaryValueLetting(
                    Expression::Atomic(Metadata::new(), Atom::Literal(lit.clone())),
                ));
            previous.push((target.clone(), previous_kind));
        }

        QuantifiedBinding { targets: previous }
    }
}

impl Drop for QuantifiedBinding {
    fn drop(&mut self) {
        for (target, previous_kind) in &self.targets {
            let mut mutable_target = target.clone();
            let _ = mutable_target.replace_kind(previous_kind.clone());
        }
    }
}
