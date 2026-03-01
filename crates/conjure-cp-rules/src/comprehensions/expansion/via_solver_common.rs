use std::{
    collections::HashMap,
    sync::{Arc, RwLock},
};

use conjure_cp::{
    ast::{
        Atom, DecisionVariable, DeclarationKind, DeclarationPtr, Expression, Literal, Metadata,
        Model, Name, Reference, SymbolTable, eval_constant, run_partial_evaluator,
        serde::{HasId as _, ObjId},
    },
    context::Context,
    rule_engine::{RuleSet, rewrite_morph, rewrite_naive},
    settings::Rewriter,
};
use uniplate::Biplate as _;

/// Configures a temporary model for solver-based comprehension expansion.
pub(super) fn with_temporary_model(model: Model, search_order: Option<Vec<Name>>) -> Model {
    let mut model = model;
    model.context = Arc::new(RwLock::new(Context::default()));
    model.search_order = search_order;
    model
}

/// Rewrites a model using the currently configured rewriter and Minion-oriented rule sets.
pub(super) fn rewrite_model_with_configured_rewriter<'a>(
    model: Model,
    rule_sets: &Vec<&'a RuleSet<'a>>,
    configured_rewriter: Rewriter,
) -> Model {
    match configured_rewriter {
        Rewriter::Morph => rewrite_morph(model, rule_sets, false),
        Rewriter::Naive => rewrite_naive(&model, rule_sets, false).unwrap(),
    }
}

/// Instantiates rewritten return expressions with quantified assignments.
///
/// This does not mutate any parent symbol table.
pub(super) fn instantiate_return_expressions_from_values(
    values: Vec<HashMap<Name, Literal>>,
    return_expression_model: &Model,
    quantified_vars: &[Name],
) -> Vec<Expression> {
    let mut return_expressions = vec![];

    for value in values {
        let return_expression_model = return_expression_model.clone();
        let child_symtab = return_expression_model.symbols().clone();
        let mut return_expression = return_expression_model.into_single_expression();

        // We only bind quantified variables.
        let value: HashMap<_, _> = value
            .into_iter()
            .filter(|(name, _)| quantified_vars.contains(name))
            .collect();

        // Bind quantified references by updating declaration targets, then simplify.
        let _temp_value_bindings =
            temporarily_bind_quantified_vars_to_values(&child_symtab, &return_expression, &value);
        return_expression = concretise_resolved_reference_atoms(return_expression);
        return_expression = simplify_expression(return_expression);

        return_expressions.push(return_expression);
    }

    return_expressions
}

pub(super) fn retain_quantified_solution_values(
    mut values: HashMap<Name, Literal>,
    quantified_vars: &[Name],
) -> HashMap<Name, Literal> {
    values.retain(|name, _| quantified_vars.contains(name));
    values
}

pub(super) fn simplify_expression(mut expr: Expression) -> Expression {
    // Keep applying evaluators to a fixed point, or until no changes are made.
    for _ in 0..128 {
        let next = expr.clone().transform_bi(&|subexpr: Expression| {
            if let Some(lit) = eval_constant(&subexpr) {
                return Expression::Atomic(Metadata::new(), Atom::Literal(lit));
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
        let new_decl = parent_symtab.gensym(&decl.domain().unwrap());
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
    expr: &Expression,
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
    for decl in uniplate::Biplate::<DeclarationPtr>::universe_bi(expr) {
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
    originals: Vec<(DeclarationPtr, DeclarationKind)>,
}

impl Drop for TempQuantifiedFindGuard {
    fn drop(&mut self) {
        for (mut decl, kind) in self.originals.drain(..) {
            let _ = decl.replace_kind(kind);
        }
    }
}

/// Converts quantified declarations in `model` to temporary find declarations.
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

        let new_kind = DeclarationKind::Find(DecisionVariable::new(domain));
        let _ = decl.replace_kind(new_kind);
        originals.push((decl, old_kind));
    }

    TempQuantifiedFindGuard { originals }
}
