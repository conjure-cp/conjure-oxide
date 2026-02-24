use std::sync::{Arc, Mutex};

use conjure_cp::{
    ast::{Expression, SymbolTable, comprehension::Comprehension},
    rule_engine::resolve_rule_sets,
    settings::{SolverFamily, current_rewriter},
    solver::{Solver, SolverError, adaptors::Minion},
};

use super::via_solver_common::{
    instantiate_return_expressions_from_values, model_from_submodel,
    rewrite_model_with_configured_rewriter, temporarily_materialise_quantified_vars_as_finds,
};

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

    // only branch on the quantified variables.
    let generator_model = model_from_submodel(
        comprehension.generator_submodel.clone(),
        Some(comprehension.quantified_vars.clone()),
    );

    // call rewrite here as well as in expand_via_solver_ac, just to be consistent
    let extra_rule_sets = &["Base", "Constant", "Bubble"];

    let rule_sets = resolve_rule_sets(SolverFamily::Minion, extra_rule_sets).unwrap();
    let configured_rewriter = current_rewriter();

    let generator_model =
        rewrite_model_with_configured_rewriter(generator_model, &rule_sets, configured_rewriter);

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

    let return_expression_model = rewrite_model_with_configured_rewriter(
        model_from_submodel(comprehension.return_expression_submodel.clone(), None),
        &rule_sets,
        configured_rewriter,
    );

    let solver_model = generator_model.clone();

    // Minion expects quantified variables in the temporary generator model as find declarations.
    // Keep this conversion local to the model passed into Minion.
    let _temp_finds = temporarily_materialise_quantified_vars_as_finds(
        solver_model.as_submodel(),
        &comprehension.quantified_vars,
    );

    let minion = minion.load_model(solver_model)?;

    let values = Arc::new(Mutex::new(Vec::new()));
    let values_ptr = Arc::clone(&values);

    tracing::debug!(model=%generator_model,comprehension=%comprehension,"Minion solving comprehension (solver mode)");
    minion.solve(Box::new(move |sols| {
        // TODO: deal with represented names if quantified variables are abslits.
        let values = &mut *values_ptr.lock().unwrap();
        values.push(sols);
        true
    }))?;

    let values = values.lock().unwrap().clone();
    Ok(instantiate_return_expressions_from_values(
        values,
        &return_expression_model,
        &comprehension.quantified_vars,
        symtab,
    ))
}
