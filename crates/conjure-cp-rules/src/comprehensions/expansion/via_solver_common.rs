use std::{
    collections::HashMap,
    sync::{Arc, RwLock},
};

use conjure_cp::{
    ast::{
        Atom, DecisionVariable, DeclarationKind, DeclarationPtr, Expression, Literal, Model, Name,
        Reference, SubModel, SymbolTable,
        serde::{HasId as _, ObjId},
    },
    bug,
    context::Context,
    rule_engine::{RuleSet, rewrite_morph, rewrite_naive},
    settings::Rewriter,
};
use uniplate::Biplate as _;

/// Creates a temporary model wrapping the given submodel.
pub(super) fn model_from_submodel(submodel: SubModel, search_order: Option<Vec<Name>>) -> Model {
    let mut model = Model::new(Arc::new(RwLock::new(Context::default())));
    *model.as_submodel_mut() = submodel;
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
        Rewriter::Naive => rewrite_naive(&model, rule_sets, false, false).unwrap(),
    }
}

/// Instantiates rewritten return expressions with solver assignments and lifts machine symbols
/// into parent scope.
pub(super) fn instantiate_return_expressions_from_values(
    values: Vec<HashMap<Name, Literal>>,
    return_expression_model: &Model,
    quantified_vars: &[Name],
    symtab: &mut SymbolTable,
) -> Vec<Expression> {
    let mut return_expressions = vec![];

    for value in values {
        let return_expression_submodel = return_expression_model.as_submodel().clone();
        let child_symtab = return_expression_submodel.symbols().clone();
        let return_expression = return_expression_submodel.into_single_expression();

        // We only substitute quantified variables.
        let value: HashMap<_, _> = value
            .into_iter()
            .filter(|(name, _)| quantified_vars.contains(name))
            .collect();

        let return_expression = return_expression.transform_bi(&|x: Atom| {
            let Atom::Reference(ref ptr) = x else {
                return x;
            };

            let Some(lit) = value.get(&ptr.name()) else {
                return x;
            };

            Atom::Literal(lit.clone())
        });

        let mut machine_name_translations: HashMap<ObjId, DeclarationPtr> = HashMap::new();

        for (name, decl) in child_symtab.into_iter_local() {
            // Do not add quantified declarations for quantified vars to the parent symbol table.
            if value.get(&name).is_some()
                && matches!(
                    &decl.kind() as &DeclarationKind,
                    DeclarationKind::Quantified(_)
                )
            {
                continue;
            }

            let Name::Machine(_) = &name else {
                bug!(
                    "the symbol table of the return expression of a comprehension should only contain machine names"
                );
            };

            let id = decl.id();
            let new_decl = symtab.gensym(&decl.domain().unwrap());
            machine_name_translations.insert(id, new_decl);
        }

        let return_expression = return_expression.transform_bi(&|atom: Atom| {
            if let Atom::Reference(ref decl) = atom
                && let id = decl.id()
                && let Some(new_decl) = machine_name_translations.get(&id)
            {
                Atom::Reference(Reference::new(new_decl.clone()))
            } else {
                atom
            }
        });

        return_expressions.push(return_expression);
    }

    return_expressions
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

/// Converts quantified declarations in `submodel` to temporary find declarations.
pub(super) fn temporarily_materialise_quantified_vars_as_finds(
    submodel: &SubModel,
    quantified_vars: &[Name],
) -> TempQuantifiedFindGuard {
    let symbols = submodel.symbols().clone();
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
