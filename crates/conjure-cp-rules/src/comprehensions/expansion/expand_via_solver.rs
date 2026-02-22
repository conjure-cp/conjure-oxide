use std::{
    collections::HashMap,
    sync::{Arc, Mutex, RwLock, atomic::Ordering},
};

use conjure_cp::{
    ast::{
        Atom, DecisionVariable, DeclarationKind, DeclarationPtr, Expression, Model, Name, SubModel,
        SymbolTable,
        comprehension::{Comprehension, USE_OPTIMISED_REWRITER_FOR_COMPREHENSIONS},
        serde::{HasId as _, ObjId},
    },
    bug,
    context::Context,
    rule_engine::{resolve_rule_sets, rewrite_morph, rewrite_naive},
    settings::SolverFamily,
    solver::{Solver, SolverError, adaptors::Minion},
};
use uniplate::Biplate as _;

/// Expands the comprehension by solving quantified variables with Minion.
///
/// This returns one expression per assignment to quantified variables that satisfies the static
/// guards of the comprehension.
///
/// If successful, this modifies the symbol table given to add aux-variables needed inside the
/// expanded expressions.
pub fn expand_via_solver(
    comprehension: Comprehension,
    symtab: &mut SymbolTable,
) -> Result<Vec<Expression>, SolverError> {
    let minion = Solver::new(Minion::new());
    // FIXME: weave proper context through
    let mut model = Model::new(Arc::new(RwLock::new(Context::default())));

    // only branch on the quantified variables.
    model.search_order = Some(comprehension.quantified_vars.clone());
    *model.as_submodel_mut() = comprehension.generator_submodel.clone();

    // call rewrite here as well as in expand_via_solver_ac, just to be consistent
    let extra_rule_sets = &["Base", "Constant", "Bubble"];

    let rule_sets = resolve_rule_sets(SolverFamily::Minion, extra_rule_sets).unwrap();

    let model = if USE_OPTIMISED_REWRITER_FOR_COMPREHENSIONS.load(Ordering::Relaxed) {
        rewrite_morph(model, &rule_sets, false)
    } else {
        rewrite_naive(&model, &rule_sets, false, false).unwrap()
    };

    // Call the rewriter to rewrite inside the comprehension
    //
    // The original idea was to let the top level rewriter rewrite the return expression model
    // and the generator model. The comprehension wouldn't be expanded until the generator
    // model is in valid minion that can be ran, at which point the return expression model
    // should also be in valid minion.
    //
    // By calling the rewriter inside the rule, we no longer wait for the generator model to be
    // valid Minion, so we don't get the simplified return model either...
    //
    // We need to do this as we want to modify the generator model (add the dummy Z's) then
    // solve and return in one go.
    //
    // Comprehensions need a big rewrite soon, as theres lots of sharp edges such as this in
    // my original implementation, and I don't think we can fit our new optimisation into it.
    // If we wanted to avoid calling the rewriter, we would need to run the first half the rule
    // up to adding the return expr to the generator model, yield, then come back later to
    // actually solve it?

    let return_expression_submodel = comprehension.return_expression_submodel.clone();
    let mut return_expression_model = Model::new(Arc::new(RwLock::new(Context::default())));
    *return_expression_model.as_submodel_mut() = return_expression_submodel;

    let return_expression_model =
        if USE_OPTIMISED_REWRITER_FOR_COMPREHENSIONS.load(Ordering::Relaxed) {
            rewrite_morph(return_expression_model, &rule_sets, false)
        } else {
            rewrite_naive(&return_expression_model, &rule_sets, false, false).unwrap()
        };

    let solver_model = model.clone();

    // Minion expects quantified variables in the temporary generator model as find declarations.
    // Keep this conversion local to the model passed into Minion.
    let _temp_finds = temporarily_materialise_quantified_vars_as_finds(
        solver_model.as_submodel(),
        &comprehension.quantified_vars,
    );

    let minion = minion.load_model(solver_model)?;

    let values = Arc::new(Mutex::new(Vec::new()));
    let values_ptr = Arc::clone(&values);

    tracing::debug!(model=%model,comprehension=%comprehension,"Minion solving comprehension (solver mode)");
    minion.solve(Box::new(move |sols| {
        // TODO: deal with represented names if quantified variables are abslits.
        let values = &mut *values_ptr.lock().unwrap();
        values.push(sols);
        true
    }))?;

    let values = values.lock().unwrap().clone();

    let mut return_expressions = vec![];

    for value in values {
        // convert back to an expression

        let return_expression_submodel = return_expression_model.as_submodel().clone();
        let child_symtab = return_expression_submodel.symbols().clone();
        let return_expression = return_expression_submodel.into_single_expression();

        // we only want to substitute quantified variables.
        // (definitely not machine names, as they mean something different in this scope!)
        let value: HashMap<_, _> = value
            .into_iter()
            .filter(|(n, _)| comprehension.quantified_vars.contains(n))
            .collect();

        let value_ptr = Arc::new(value);
        let value_ptr_2 = Arc::clone(&value_ptr);

        // substitute in the values for the quantified variables
        let return_expression = return_expression.transform_bi(&move |x: Atom| {
            let Atom::Reference(ref ptr) = x else {
                return x;
            };

            // is this referencing a quantified var?
            let Some(lit) = value_ptr_2.get(&ptr.name()) else {
                return x;
            };

            Atom::Literal(lit.clone())
        });

        // Copy the return expression's symbols into parent scope.

        // For variables in the return expression with machine names, create new declarations
        // for them in the parent symbol table, so that the machine names used are unique.
        //
        // Store the declaration translations in `machine_name_translations`.
        // These are stored as a map of (old declaration id) -> (new declaration ptr), as
        // declaration pointers do not implement hash.
        //
        let mut machine_name_translations: HashMap<ObjId, DeclarationPtr> = HashMap::new();

        // Populate `machine_name_translations`
        for (name, decl) in child_symtab.into_iter_local() {
            // do not add quantified declarations for quantified vars to the parent symbol table.
            if value_ptr.get(&name).is_some()
                && matches!(
                    &decl.kind() as &DeclarationKind,
                    DeclarationKind::Given(_) | DeclarationKind::Quantified(_)
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

        // Update references to use the new delcarations.
        #[allow(clippy::arc_with_non_send_sync)]
        let return_expression = return_expression.transform_bi(&move |atom: Atom| {
            if let Atom::Reference(ref decl) = atom
                && let id = decl.id()
                && let Some(new_decl) = machine_name_translations.get(&id)
            {
                Atom::Reference(conjure_cp::ast::Reference::new(new_decl.clone()))
            } else {
                atom
            }
        });

        return_expressions.push(return_expression);
    }

    Ok(return_expressions)
}

/// Guard that temporarily converts quantified declarations to find declarations.
struct TempQuantifiedFindGuard {
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
fn temporarily_materialise_quantified_vars_as_finds(
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
