use std::{
    collections::VecDeque,
    sync::{Arc, Mutex},
};

use conjure_cp::{
    ast::{
        Atom, DeclarationPtr, Expression, Metadata, Moo, Name, ReturnType, SubModel, SymbolTable,
        Typeable as _, ac_operators::ACOperatorKind, comprehension::Comprehension,
    },
    rule_engine::resolve_rule_sets,
    settings::{SolverFamily, current_rewriter},
    solver::{Solver, SolverError, adaptors::Minion},
};
use tracing::warn;
use uniplate::{Biplate, Uniplate as _, zipper::Zipper};

use super::via_solver_common::{
    instantiate_return_expressions_from_values, model_from_submodel,
    rewrite_model_with_configured_rewriter, temporarily_materialise_quantified_vars_as_finds,
};

/// Expands the comprehension using Minion, returning the resulting expressions.
///
/// This method is only suitable for comprehensions inside an AC operator. The AC operator that
/// contains this comprehension should be passed into the `ac_operator` argument.
///
/// This method performs additional pruning of "uninteresting" values, only possible when the
/// comprehension is inside an AC operator.
///
/// If successful, this modifies the symbol table given to add aux-variables needed inside the
/// expanded expressions.
pub fn expand_via_solver_ac(
    comprehension: Comprehension,
    symtab: &mut SymbolTable,
    ac_operator: ACOperatorKind,
) -> Result<Vec<Expression>, SolverError> {
    // ADD RETURN EXPRESSION TO GENERATOR MODEL AS CONSTRAINT
    // ======================================================

    // References to quantified variables in the return expression point to entries in the
    // return_expression symbol table.
    //
    // Change these to point to the corresponding entry in the generator symbol table instead.
    let quantified_vars_2 = comprehension.quantified_vars.clone();
    let generator_symtab_ptr = comprehension.generator_submodel.symbols_ptr_unchecked();
    let return_expression =
        comprehension
            .clone()
            .return_expression()
            .transform_bi(&move |decl: DeclarationPtr| {
                // if this variable is a quantified var...
                if quantified_vars_2.contains(&decl.name()) {
                    // ... use the generator symbol tables version of it

                    generator_symtab_ptr
                        .read()
                        .lookup_local(&decl.name())
                        .unwrap()
                } else {
                    decl
                }
            });

    // Replace all boolean expressions referencing non-quantified variables in the return
    // expression with dummy variables. This allows us to add it as a constraint to the
    // generator model.
    let generator_submodel = add_return_expression_to_generator_model(
        comprehension.generator_submodel.clone(),
        return_expression,
        &ac_operator,
    );

    // REWRITE GENERATOR MODEL AND PASS TO MINION
    // ==========================================

    let generator_model = model_from_submodel(
        generator_submodel,
        Some(comprehension.quantified_vars.clone()),
    );

    let extra_rule_sets = &["Base", "Constant", "Bubble"];

    let rule_sets = resolve_rule_sets(SolverFamily::Minion, extra_rule_sets).unwrap();
    let configured_rewriter = current_rewriter();

    // In AC mode we materialise quantified variables before rewriting, as the rewritten
    // generator model is used directly as Minion input.
    let _temp_finds = temporarily_materialise_quantified_vars_as_finds(
        generator_model.as_submodel(),
        &comprehension.quantified_vars,
    );

    let generator_model =
        rewrite_model_with_configured_rewriter(generator_model, &rule_sets, configured_rewriter);

    let minion = Solver::new(Minion::new());
    let minion = minion.load_model(generator_model.clone());

    let minion = match minion {
        Err(e) => {
            warn!(why=%e,model=%generator_model,"Loading generator model failed, failing solver-backed AC comprehension expansion rule");
            return Err(e);
        }
        Ok(minion) => minion,
    };

    // REWRITE RETURN EXPRESSION
    // =========================

    let return_expression_model = rewrite_model_with_configured_rewriter(
        model_from_submodel(comprehension.return_expression_submodel.clone(), None),
        &rule_sets,
        configured_rewriter,
    );

    let values = Arc::new(Mutex::new(Vec::new()));
    let values_ptr = Arc::clone(&values);

    // SOLVE FOR THE QUANTIFIED VARIABLES, AND SUBSTITUTE INTO THE REWRITTEN RETURN EXPRESSION
    // ======================================================================================

    tracing::debug!(model=%generator_model,comprehension=%comprehension,"Minion solving comprehnesion (ac mode)");

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

/// Eliminate all references to non-quantified variables by introducing dummy variables to the
/// return expression. This modified return expression is added to the generator model, which is
/// returned.
///
/// Dummy variables must be the same type as the AC operators identity value.
///
/// To reduce the number of dummy variables, we turn the largest expression containing only
/// non-quantified variables and of the correct type into a dummy variable.
///
/// If there is no such expression, (e.g. and[(a<i) | i: int(1..10)]) , we use the smallest
/// expression of the correct type that contains a non-quantified variable. This ensures that
/// we lose as few references to quantified variables as possible.
fn add_return_expression_to_generator_model(
    mut generator_submodel: SubModel,
    return_expression: Expression,
    ac_operator: &ACOperatorKind,
) -> SubModel {
    let mut zipper = Zipper::new(return_expression);
    let mut symtab = generator_submodel.symbols_mut();

    // for sum/product we want to put integer expressions into dummy variables,
    // for and/or we want to put boolean expressions into dummy variables.
    let dummy_var_type = ac_operator.identity().return_type();

    'outer: loop {
        let focus: &mut Expression = zipper.focus_mut();

        let (non_quantified_vars, quantified_vars) = partition_variables(focus, &symtab);

        // an expression or its descendants needs to be turned into a dummy variable if it
        // contains non-quantified variables.
        let has_non_quantified_vars = !non_quantified_vars.is_empty();

        // does this expression contain quantified variables?
        let has_quantified_vars = !quantified_vars.is_empty();

        // can this expression be turned into a dummy variable?
        let can_be_dummy_var = can_be_dummy_variable(focus, &dummy_var_type);

        // The expression and its descendants don't need a dummy variables, so we don't
        // need to descend into its children.
        if !has_non_quantified_vars {
            // go to next node or quit
            while zipper.go_right().is_none() {
                let Some(()) = zipper.go_up() else {
                    // visited all nodes
                    break 'outer;
                };
            }
            continue;
        }

        // The expression contains non-quantified variables:

        // does this expression have any children that can be turned into dummy variables?
        let has_eligible_child = focus.universe().iter().skip(1).any(|expr| {
            // eligible if it can be turned into a dummy variable, and turning it into a
            // dummy variable removes a non-quantified variable from the model.
            can_be_dummy_variable(expr, &dummy_var_type)
                && contains_non_quantified_variables(expr, &symtab)
        });

        // This expression has no child that can be turned into a dummy variable, but can
        // be a dummy variable => turn it into a dummy variable and continue.
        if !has_eligible_child && can_be_dummy_var {
            // introduce dummy var and continue
            let dummy_domain = focus.domain_of().unwrap();
            let dummy_decl = symtab.gensym(&dummy_domain);
            *focus = Expression::Atomic(
                Metadata::new(),
                Atom::Reference(conjure_cp::ast::Reference::new(dummy_decl)),
            );

            // go to next node
            while zipper.go_right().is_none() {
                let Some(()) = zipper.go_up() else {
                    // visited all nodes
                    break 'outer;
                };
            }
            continue;
        }
        // This expression has no child that can be turned into a dummy variable, and
        // cannot be a dummy variable => backtrack upwards to find a parent that can be a
        // dummy variable, and make it a dummy variable.
        else if !has_eligible_child && !can_be_dummy_var {
            // TODO: remove this case, make has_eligible_child check better?

            // go upwards until we find something that can be a dummy variable, make it
            // a dummy variable, then continue.
            while let Some(()) = zipper.go_up() {
                let focus = zipper.focus_mut();
                if can_be_dummy_variable(focus, &dummy_var_type) {
                    // TODO: this expression we are rewritng might already contain
                    // dummy vars - we might need a pass to get rid of the unused
                    // ones!
                    //
                    // introduce dummy var and continue
                    let dummy_domain = focus.domain_of().unwrap();
                    let dummy_decl = symtab.gensym(&dummy_domain);
                    *focus = Expression::Atomic(
                        Metadata::new(),
                        Atom::Reference(conjure_cp::ast::Reference::new(dummy_decl)),
                    );

                    // go to next node
                    while zipper.go_right().is_none() {
                        let Some(()) = zipper.go_up() else {
                            // visited all nodes
                            break 'outer;
                        };
                    }
                    continue;
                }
            }
            unreachable!();
        }
        // If the expression contains quantified variables as well as non-quantified
        // variables, try to retain the quantified variables by finding a child that can be
        // made a dummy variable which has only non-quantified variables.
        else if has_eligible_child && has_quantified_vars {
            zipper
                .go_down()
                .expect("we know the focus has a child, so zipper.go_down() should succeed");
        }
        // This expression contains no quantified variables, so no point trying to turn a
        // child into a dummy variable.
        else if has_eligible_child && !has_quantified_vars {
            // introduce dummy var and continue
            let dummy_domain = focus.domain_of().unwrap();
            let dummy_decl = symtab.gensym(&dummy_domain);
            *focus = Expression::Atomic(
                Metadata::new(),
                Atom::Reference(conjure_cp::ast::Reference::new(dummy_decl)),
            );

            // go to next node
            while zipper.go_right().is_none() {
                let Some(()) = zipper.go_up() else {
                    // visited all nodes
                    break 'outer;
                };
            }
        } else {
            unreachable!()
        }
    }

    let new_return_expression = Expression::Neq(
        Metadata::new(),
        Moo::new(Expression::Atomic(
            Metadata::new(),
            ac_operator.identity().into(),
        )),
        Moo::new(zipper.rebuild_root()),
    );

    // double check that the above transformation didn't miss any stray non-quantified vars
    assert!(
        Biplate::<DeclarationPtr>::universe_bi(&new_return_expression)
            .iter()
            .all(|x| symtab.lookup_local(&x.name()).is_some()),
        "generator model should only contain references to variables in its symbol table."
    );

    std::mem::drop(symtab);

    generator_submodel.add_constraint(new_return_expression);

    generator_submodel
}

/// Returns a tuple of non-quantified decision variables and quantified variables inside the expression.
///
/// As lettings, givens, etc. will eventually be subsituted for constants, this only returns
/// non-quantified _decision_ variables.
#[inline]
fn partition_variables(
    expr: &Expression,
    symtab: &SymbolTable,
) -> (VecDeque<Name>, VecDeque<Name>) {
    // doing this as two functions non_quantified_variables and quantified_variables might've been
    // easier to read.
    //
    // However, doing this in one function avoids an extra universe call...
    let (non_quantified_vars, quantified_vars): (VecDeque<Name>, VecDeque<Name>) =
        Biplate::<Name>::universe_bi(expr)
            .into_iter()
            .partition(|x| symtab.lookup_local(x).is_none());

    (non_quantified_vars, quantified_vars)
}

/// Returns `true` if `expr` can be turned into a dummy variable.
#[inline]
fn can_be_dummy_variable(expr: &Expression, dummy_variable_type: &ReturnType) -> bool {
    // do not put root expression in a dummy variable or things go wrong.
    if matches!(expr, Expression::Root(_, _)) {
        return false;
    };

    // is the expression the same type as the dummy variable?
    expr.return_type() == *dummy_variable_type
}

/// Returns `true` if `expr` or its descendants contains non-quantified variables.
#[inline]
fn contains_non_quantified_variables(expr: &Expression, symtab: &SymbolTable) -> bool {
    let names_referenced: VecDeque<Name> = expr.universe_bi();
    // a name is a non-quantified variable if its definition is not in the local scope of the
    // comprehension's generators.
    names_referenced
        .iter()
        .any(|x| symtab.lookup_local(x).is_none())
}
