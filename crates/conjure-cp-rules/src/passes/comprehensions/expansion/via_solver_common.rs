use std::{
    collections::HashMap,
    sync::{Arc, RwLock},
};

use crate::types::matrix::try_lower_const_unsafe_index_matrix_components;
use conjure_cp::{
    ast::{
        Atom, DecisionVariable, DeclarationKind, DeclarationPtr, Expression, GroundDomain, Literal,
        Metadata, Model, Name, Range, Reference, SymbolTable,
        ac_operators::ACOperatorKind,
        comprehension::{Comprehension, ComprehensionQualifier},
        eval_constant, run_partial_evaluator,
        serde::{HasId as _, ObjId},
    },
    bug,
    context::Context,
    representation::{ReprStore, util::try_up},
    rule_engine::{
        RuleSet,
        rewrite_model_with_configured_rewriter as rewrite_model_with_configured_rewriter_core,
    },
    settings::{Rewriter, with_compact_heuristic},
    solver::SolverError,
};
use uniplate::{Biplate as _, Uniplate as _};

/// Configures a temporary model for solver-based comprehension expansion.
pub(super) fn with_temporary_model(model: Model, search_order: Option<Vec<Name>>) -> Model {
    let mut model = model;
    model.context = Arc::new(RwLock::new(Context::default()));
    model.search_order = search_order;
    model
}

/// Rewrites a temporary model using the currently configured rewriter and Minion-oriented rule
/// sets.
///
/// Representations for a throwaway model are chosen compactly rather than by the configured
/// heuristic -- see [`with_compact_heuristic`].
pub(super) fn rewrite_model_with_configured_rewriter<'a>(
    model: Model,
    rule_sets: &Vec<&'a RuleSet<'a>>,
    configured_rewriter: Rewriter,
) -> Model {
    with_compact_heuristic(|| {
        rewrite_model_with_configured_rewriter_core(model, rule_sets, configured_rewriter).unwrap()
    })
}

/// Splits out the guards that reference non-quantified decision variables.
///
/// These guards cannot be decided while expanding: the outer decision variables they mention do
/// not belong to the generator submodel, so putting them in it both asks Minion a meaningless
/// question and leaves undeclared names (e.g. the components of a represented outer matrix) in
/// the model handed to it. They are re-applied to each expanded element instead, through the
/// enclosing AC operator's skip semantics -- the same treatment the native expander gives them.
///
/// Returns the comprehension with those guards removed, alongside the guards themselves.
pub(super) fn split_symbolic_guards(
    comprehension: &Comprehension,
) -> (Comprehension, Vec<Expression>) {
    let mut symbolic_guards = vec![];
    let mut qualifiers = vec![];

    for qualifier in &comprehension.qualifiers {
        match qualifier {
            ComprehensionQualifier::Condition(condition)
                if !comprehension.is_quantified_guard(condition) =>
            {
                symbolic_guards.push(condition.clone());
            }
            qualifier => qualifiers.push(qualifier.clone()),
        }
    }

    let mut stripped = comprehension.clone();
    stripped.qualifiers = qualifiers;
    (stripped, symbolic_guards)
}

/// The domain bound to substitute for a guarded-out element in a min/max skip operation: the
/// return expression's own upper bound (for `min`) or lower bound (for `max`). Minion requires
/// every decision variable to have a finite, bounded domain, so this is always expected to
/// resolve; if it somehow doesn't, that's a genuine "can't expand this comprehension" failure
/// (not a bug to hide), surfaced as a model-feature error rather than a panic.
pub(super) fn min_max_skip_value(
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

/// Instantiates rewritten return expressions with quantified assignments.
///
/// `symbolic_guards` are the guards held back by [`split_symbolic_guards`]. They are instantiated
/// under the same assignment as the return expression: a guard that becomes false drops the
/// element, a guard that becomes true is discharged, and one that stays symbolic wraps the element
/// in `skip_operator`'s skip operation.
///
/// This does not mutate any parent symbol table.
pub(super) fn instantiate_return_expressions_from_values(
    values: Vec<HashMap<Name, Literal>>,
    return_expression_model: &Model,
    quantified_vars: &[Name],
    symbolic_guards: &[Expression],
    skip_operator: Option<ACOperatorKind>,
) -> Result<Vec<Expression>, SolverError> {
    let mut return_expressions = vec![];

    // As in the native expander, the safe value to substitute for a guarded-out min/max element
    // must come from the return expression's *general* domain, computed once before any quantified
    // variable is bound to a concrete value.
    let min_max_skip_value = match skip_operator {
        Some(op @ (ACOperatorKind::Min | ACOperatorKind::Max)) if !symbolic_guards.is_empty() => {
            let return_expression = return_expression_model.clone().into_single_expression();
            Some(min_max_skip_value(
                &return_expression,
                op == ACOperatorKind::Max,
            )?)
        }
        _ => None,
    };

    for value in values {
        let return_expression_model = return_expression_model.clone();
        let child_symtab = return_expression_model.symbols().clone();
        let mut return_expression = return_expression_model.into_single_expression();

        // We only bind quantified variables.
        let value: HashMap<_, _> = value
            .into_iter()
            .filter(|(name, _)| quantified_vars.contains(name))
            .collect();

        // Bind quantified references by updating declaration targets, then simplify. The held-back
        // guards are bound alongside the return expression: they quantify over the same variables.
        let mut binding_targets = Vec::with_capacity(symbolic_guards.len() + 1);
        binding_targets.push(return_expression.clone());
        binding_targets.extend(symbolic_guards.iter().cloned());
        let _temp_value_bindings =
            temporarily_bind_quantified_vars_to_values(&child_symtab, &binding_targets, &value);

        let Some(guards) = instantiate_symbolic_guards(symbolic_guards)? else {
            // A guard is false under this assignment: the element is skipped entirely.
            continue;
        };

        return_expression = concretise_resolved_reference_atoms(return_expression);
        let Some(mut return_expression) = strip_guarded_safe_index_conditions(return_expression)
        else {
            continue;
        };
        return_expression = simplify_expression(return_expression);

        for guard in guards {
            let Some(ac_operator) = skip_operator else {
                return Err(SolverError::ModelInvalid(format!(
                    "comprehension has symbolic guard but no AC operator context for \
                     solver-backed expansion: {guard}"
                )));
            };
            return_expression = match (ac_operator, &min_max_skip_value) {
                (ACOperatorKind::Min | ACOperatorKind::Max, Some(skip_value)) => ac_operator
                    .make_min_max_skip_operation(guard, return_expression, skip_value.clone()),
                (ACOperatorKind::Min | ACOperatorKind::Max, None) => {
                    bug!("min/max comprehension expansion is missing its precomputed skip value")
                }
                _ => ac_operator.make_skip_operation(guard, return_expression),
            };
        }

        return_expressions.push(return_expression);
    }

    Ok(return_expressions)
}

/// Instantiates the held-back guards under the bindings currently in force.
///
/// Returns `Ok(None)` when a guard is false (the element is skipped), otherwise the guards that
/// are still symbolic and so have to be attached to the element.
fn instantiate_symbolic_guards(
    symbolic_guards: &[Expression],
) -> Result<Option<Vec<Expression>>, SolverError> {
    let mut remaining = vec![];

    for guard in symbolic_guards {
        let guard = simplify_expression(concretise_resolved_reference_atoms(guard.clone()));
        match eval_constant(&guard) {
            Some(Literal::Bool(true)) => {}
            Some(Literal::Bool(false)) => return Ok(None),
            Some(other) => {
                return Err(SolverError::ModelInvalid(format!(
                    "comprehension guard must evaluate to Bool, got {other}: {guard}"
                )));
            }
            None => remaining.push(guard),
        }
    }

    Ok(Some(remaining))
}

/// Keeps only the quantified assignments of a solver solution, discarding auxiliaries and locals.
///
/// A quantified variable with an abstract domain is branched on through its representation, so the
/// solver reports its representation variables rather than the variable itself. Those are lifted
/// back into one abstract value with [`try_up`], giving a value that can be substituted into the
/// return expression.
pub(super) fn retain_quantified_solution_values(
    values: HashMap<Name, Literal>,
    quantified_vars: &[Name],
    symbols: &SymbolTable,
) -> HashMap<Name, Literal> {
    let mut quantified_values = HashMap::new();

    for name in quantified_vars {
        // `try_up` reads a directly assigned value when there is one, and otherwise goes up
        // through the declaration's representation.
        let Some(decl) = symbols.lookup(name) else {
            continue;
        };
        let Ok(value) = try_up(decl, &values) else {
            continue;
        };
        quantified_values.insert(name.clone(), value);
    }

    quantified_values
}

/// Simplifies an instantiated comprehension element before it enters the rewriter worklist.
///
/// Applies constant folding and deep partial evaluation (including boolean `x = true` lowering in
/// the `Eq` arm), and lowers constant in-bounds `MatrixComponents` [`Expression::UnsafeIndex`] nodes
/// using the same oracle as `unsafe_const_index_matrix_components`, so ground tautologies and ground
/// indexing from expansion do not each pay a per-site rewriter worklist update.
pub(super) fn simplify_expression(mut expr: Expression) -> Expression {
    // Keep applying evaluators to a fixed point, or until no changes are made.
    for _ in 0..128 {
        let next = expr.clone().transform_bi(&|subexpr: Expression| {
            if let Some(lit) = eval_constant(&subexpr) {
                return Expression::Atomic(Metadata::new(), Atom::Literal(lit));
            }
            if let Some(lowered) = try_lower_const_unsafe_index_matrix_components(&subexpr) {
                return lowered;
            }
            if let Ok(reduction) = run_partial_evaluator(&subexpr) {
                return reduction.new_expression;
            }
            subexpr
        });

        if next == expr {
            break;
        }
        expr = next;
    }

    expr
}

/// Strips internal `InDomain` guards that were introduced by bubbling a boolean `SafeIndex`
/// inside a comprehension return expression.
///
/// When a source comprehension already has a guard that filters out dummy/out-of-domain values,
/// earlier rewrites can turn that filter into a conjunction like
/// `and([SafeIndex(...), __inDomain(index, domain)])`. If we instantiate that directly, a
/// false `__inDomain` becomes a literal `false` element, which changes the comprehension from
/// "skip this element" to "include false".
///
/// We recover the original filtering behaviour only for this narrow internal pattern:
/// a top-level conjunction with exactly one non-guard term and one or more `InDomain` guards
/// that constrain indices used by that term. If any such guard is false after instantiation,
/// the element is skipped entirely.
pub(super) fn strip_guarded_safe_index_conditions(expr: Expression) -> Option<Expression> {
    let mut conjuncts = Vec::new();
    collect_top_level_and_terms(expr.clone(), &mut conjuncts);

    if conjuncts.len() == 1 && conjuncts[0] == expr {
        return Some(expr);
    }

    let (guards, mut non_guards): (Vec<_>, Vec<_>) =
        conjuncts.into_iter().partition(is_indomain_guard);

    if guards.is_empty() || non_guards.len() != 1 {
        return Some(expr);
    }

    let guarded_term = non_guards.pop().expect("length checked above");

    if !guards
        .iter()
        .all(|guard| guard_targets_safe_index_index(guard, &guarded_term))
    {
        return Some(expr);
    }

    for guard in &guards {
        let simplified_guard = simplify_expression(guard.clone());
        match eval_constant(&simplified_guard) {
            Some(Literal::Bool(true)) => {}
            Some(Literal::Bool(false)) => return None,
            _ => return Some(expr),
        }
    }

    Some(guarded_term)
}

fn collect_top_level_and_terms(expr: Expression, out: &mut Vec<Expression>) {
    if let Expression::And(_, ref children) = expr
        && let Some(children) = children.as_ref().clone().unwrap_list()
    {
        for child in children {
            collect_top_level_and_terms(child, out);
        }
    } else {
        out.push(expr);
    }
}

fn is_indomain_guard(expr: &Expression) -> bool {
    matches!(expr, Expression::InDomain(_, _, _))
}

fn guard_targets_safe_index_index(guard: &Expression, expr: &Expression) -> bool {
    let Expression::InDomain(_, guarded_index, _) = guard else {
        return false;
    };

    expr.universe().into_iter().any(|subexpr| {
        let Expression::SafeIndex(_, _, indices) = subexpr else {
            return false;
        };

        indices.iter().any(|index| index == guarded_index.as_ref())
    })
}

fn concretise_resolved_reference_atoms(expr: Expression) -> Expression {
    expr.transform_bi(&|atom: Atom| match atom {
        Atom::Reference(reference) => reference
            .resolve_constant()
            .map_or_else(|| Atom::Reference(reference), Atom::Literal),
        other => other,
    })
}

pub(super) fn lift_machine_references_into_parent_scope(
    expr: Expression,
    child_symtab: &SymbolTable,
    parent_symtab: &mut SymbolTable,
) -> Expression {
    let mut machine_name_translations: HashMap<ObjId, DeclarationPtr> = HashMap::new();

    for (name, decl) in child_symtab.clone().into_iter_local() {
        // Do not add quantified declarations for quantified vars to the parent symbol table.
        if matches!(
            &decl.kind() as &DeclarationKind,
            DeclarationKind::Quantified(_)
        ) {
            continue;
        }

        if !matches!(&name, Name::Machine(_)) {
            continue;
        }

        let id = decl.id();
        let new_decl = parent_symtab.gen_find_auxiliary(&decl.domain().unwrap());
        machine_name_translations.insert(id, new_decl);
    }

    expr.transform_bi(&|atom: Atom| {
        if let Atom::Reference(ref decl) = atom
            && let id = decl.id()
            && let Some(new_decl) = machine_name_translations.get(&id)
        {
            Atom::Reference(Reference::new(new_decl.clone()))
        } else {
            atom
        }
    })
}

/// Guard that temporarily converts quantified declarations to temporary value-lettings.
struct TempQuantifiedValueLettingGuard {
    originals: Vec<(DeclarationPtr, DeclarationKind)>,
}

impl Drop for TempQuantifiedValueLettingGuard {
    fn drop(&mut self) {
        for (mut decl, kind) in self.originals.drain(..) {
            let _ = decl.replace_kind(kind);
        }
    }
}

fn maybe_bind_temp_value_letting(
    originals: &mut Vec<(DeclarationPtr, DeclarationKind)>,
    decl: &DeclarationPtr,
    lit: &Literal,
) {
    if originals
        .iter()
        .any(|(existing, _)| existing.id() == decl.id())
    {
        return;
    }

    let mut decl = decl.clone();
    let old_kind = decl.kind().clone();
    let temp_kind = DeclarationKind::TemporaryValueLetting(Expression::Atomic(
        Metadata::new(),
        Atom::Literal(lit.clone()),
    ));
    let _ = decl.replace_kind(temp_kind);
    originals.push((decl, old_kind));
}

fn temporarily_bind_quantified_vars_to_values(
    symbols: &SymbolTable,
    exprs: &[Expression],
    values: &HashMap<Name, Literal>,
) -> TempQuantifiedValueLettingGuard {
    let mut originals = Vec::new();

    for (name, lit) in values {
        let Some(decl) = symbols.lookup_local(name) else {
            continue;
        };

        maybe_bind_temp_value_letting(&mut originals, &decl, lit);

        let kind = decl.kind();
        if let DeclarationKind::Quantified(inner) = &*kind
            && let Some(generator) = inner.generator()
        {
            maybe_bind_temp_value_letting(&mut originals, generator, lit);
        }
    }

    // Some expressions can still reference quantified declarations from an earlier scope
    // (e.g. after comprehension rewrites that rebuild generator declarations). Bind those
    // declaration pointers directly as well.
    for decl in exprs
        .iter()
        .flat_map(uniplate::Biplate::<DeclarationPtr>::universe_bi)
    {
        let name = decl.name().clone();
        let Some(lit) = values.get(&name) else {
            continue;
        };

        maybe_bind_temp_value_letting(&mut originals, &decl, lit);

        let kind = decl.kind();
        if let DeclarationKind::Quantified(inner) = &*kind
            && let Some(generator) = inner.generator()
        {
            maybe_bind_temp_value_letting(&mut originals, generator, lit);
        }
    }

    TempQuantifiedValueLettingGuard { originals }
}

/// Guard that temporarily converts quantified declarations to find declarations.
pub(super) struct TempQuantifiedFindGuard {
    originals: Vec<(DeclarationPtr, DeclarationKind, ReprStore)>,
}

impl Drop for TempQuantifiedFindGuard {
    fn drop(&mut self) {
        for (mut decl, kind, reprs) in self.originals.drain(..) {
            let _ = decl.replace_kind(kind);
            *decl.reprs_mut() = reprs;
        }
    }
}

/// Converts quantified declarations in `model` to temporary find declarations.
///
/// Declarations are shared, so the guard also restores the representations they carried. Rewriting
/// the temporary model picks a representation for each of these finds and records it on the
/// declaration; left there, the next temporary model built from the same comprehension scope would
/// start from a variable already claiming to be represented, while its representation variables
/// belong to the previous model and constrain nothing here -- so the solver is free to assign them
/// anything, and every such assignment comes back as another copy of the same expanded element.
pub(super) fn temporarily_materialise_quantified_vars_as_finds(
    model: &Model,
    quantified_vars: &[Name],
) -> TempQuantifiedFindGuard {
    let symbols = model.symbols().clone();
    let mut originals = Vec::new();

    for name in quantified_vars {
        let Some(mut decl) = symbols.lookup_local(name) else {
            continue;
        };

        let old_kind = decl.kind().clone();
        let Some(domain) = decl.domain() else {
            continue;
        };
        let old_reprs = decl.reprs().clone();

        let new_kind = DeclarationKind::Find(DecisionVariable::new(domain));
        let _ = decl.replace_kind(new_kind);
        originals.push((decl, old_kind, old_reprs));
    }

    TempQuantifiedFindGuard { originals }
}
