use std::{
    cell::RefCell,
    collections::{HashMap, VecDeque},
    sync::{Arc, Mutex},
};

use conjure_cp::{
    ast::{
        Atom, DeclarationPtr, Expression, Metadata, Moo, Name, Reference, ReturnType, SubModel,
        SymbolTable, Typeable as _,
        ac_operators::ACOperatorKind,
        comprehension::{Comprehension, ComprehensionQualifier},
        serde::HasId as _,
    },
    rule_engine::resolve_rule_sets,
    settings::{SolverFamily, current_rewriter},
    solver::{Solver, SolverError, adaptors::Minion},
};
use tracing::warn;
use uniplate::{Biplate, Uniplate as _, zipper::Zipper};

use super::via_solver_common::{
    instantiate_return_expressions_from_values, model_from_submodel,
    retain_quantified_solution_values, rewrite_model_with_configured_rewriter,
    temporarily_materialise_quantified_vars_as_finds,
};

/// Expands the comprehension using Minion, returning the resulting expressions.
///
/// This method is only suitable for comprehensions inside an AC operator. The AC operator that
/// contains this comprehension should be passed into the `ac_operator` argument.
///
/// This method performs additional pruning of "uninteresting" values, only possible when the
/// comprehension is inside an AC operator.
pub fn expand_via_solver_ac(
    comprehension: Comprehension,
    ac_operator: ACOperatorKind,
) -> Result<Vec<Expression>, SolverError> {
    let quantified_vars = comprehension.quantified_vars();

    // ADD RETURN EXPRESSION TO GENERATOR MODEL AS CONSTRAINT
    // ======================================================
    let return_expression = comprehension.return_expression.clone();

    // Replace all boolean expressions referencing non-quantified variables in the return
    // expression with dummy variables. This allows us to add it as a constraint to the
    // generator model.
    let generator_submodel = add_return_expression_to_generator_model(
        comprehension.to_generator_submodel(),
        return_expression,
        &ac_operator,
    );

    // REWRITE GENERATOR MODEL AND PASS TO MINION
    // ==========================================

    let generator_model = model_from_submodel(generator_submodel, Some(quantified_vars.clone()));

    let extra_rule_sets = &["Base", "Constant", "Bubble"];

    let rule_sets = resolve_rule_sets(SolverFamily::Minion, extra_rule_sets).unwrap();
    let configured_rewriter = current_rewriter();

    // REWRITE RETURN EXPRESSION
    // =========================

    // Keep return expressions unreduced until quantified assignments are substituted.
    // Rewriting before substitution can introduce index auxiliaries that remain symbolic and may
    // produce unsupported Minion shapes after expansion.
    let return_expression_model =
        model_from_submodel(comprehension.to_return_expression_submodel(), None);

    let values = {
        let solver_model = generator_model.clone();
        // Minion expects quantified variables in the temporary generator model as find
        // declarations. Keep this conversion scoped to solver-backed expansion.
        let _temp_finds = temporarily_materialise_quantified_vars_as_finds(
            solver_model.as_submodel(),
            &quantified_vars,
        );

        // Rewrite with quantified vars materialised as finds so Minion flattening can
        // introduce auxiliaries for constraints involving quantified variables.
        let solver_model =
            rewrite_model_with_configured_rewriter(solver_model, &rule_sets, configured_rewriter);

        let minion = Solver::new(Minion::new());
        let minion = minion.load_model(solver_model);

        let minion = match minion {
            Err(e) => {
                warn!(why=%e,model=%generator_model,"Loading generator model failed, failing solver-backed AC comprehension expansion rule");
                return Err(e);
            }
            Ok(minion) => minion,
        };

        let values = Arc::new(Mutex::new(Vec::new()));
        let values_ptr = Arc::clone(&values);
        let quantified_vars_for_solution = quantified_vars.clone();

        // SOLVE FOR THE QUANTIFIED VARIABLES, AND SUBSTITUTE INTO THE RETURN EXPRESSION
        // ============================================================================

        tracing::debug!(model=%generator_model,comprehension=%comprehension,"Minion solving comprehnesion (ac mode)");

        minion.solve(Box::new(move |sols| {
            // Only keep quantified assignments; discard solver auxiliaries/locals.
            let values = &mut *values_ptr.lock().unwrap();
            values.push(retain_quantified_solution_values(
                sols,
                &quantified_vars_for_solution,
            ));
            true
        }))?;

        values.lock().unwrap().clone()
    };
    Ok(instantiate_return_expressions_from_values(
        values,
        &return_expression_model,
        &quantified_vars,
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
    let mut symtab = generator_submodel.symbols_mut();
    let return_expression = localise_non_local_references_deep(return_expression, &mut symtab);

    let mut zipper = Zipper::new(return_expression);

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

/// Replaces references to declarations outside `symtab` with local dummy declarations.
///
/// This preserves locality by construction for temporary generator models passed to Minion.
/// All rewrites then operate purely on local references and cannot reintroduce parent-scope vars.
fn localise_non_local_references_deep(expr: Expression, symtab: &mut SymbolTable) -> Expression {
    let symtab_cell = RefCell::new(symtab);

    let expr = expr.transform_bi(&|mut comprehension: Comprehension| {
        {
            let mut symtab_borrow = symtab_cell.borrow_mut();
            let symtab_ref: &mut SymbolTable = &mut symtab_borrow;

            comprehension.return_expression = localise_non_local_references_deep(
                comprehension.return_expression.clone(),
                symtab_ref,
            );

            for qualifier in &mut comprehension.qualifiers {
                if let ComprehensionQualifier::Condition(condition) = qualifier {
                    *condition = localise_non_local_references_deep(condition.clone(), symtab_ref);
                }
            }
        }

        comprehension
    });

    let mut symtab_borrow = symtab_cell.borrow_mut();
    let symtab_ref: &mut SymbolTable = &mut symtab_borrow;
    localise_non_local_references_shallow(expr, symtab_ref)
}

fn localise_non_local_references_shallow(expr: Expression, symtab: &mut SymbolTable) -> Expression {
    let dummy_vars_by_decl_id: RefCell<HashMap<_, DeclarationPtr>> = RefCell::new(HashMap::new());
    let symtab = RefCell::new(symtab);

    expr.transform_bi(&|reference: Reference| {
        let reference_name = reference.name().clone();

        // Already local to this temporary generator model.
        if symtab.borrow().lookup_local(&reference_name).is_some() {
            return reference;
        }

        let decl = reference.ptr().clone();
        let decl_id = decl.id();

        let existing_dummy = dummy_vars_by_decl_id.borrow().get(&decl_id).cloned();
        let dummy_decl = if let Some(existing_dummy) = existing_dummy {
            existing_dummy
        } else {
            let new_dummy = {
                let domain = decl.domain().unwrap_or_else(|| {
                    panic!("non-local reference '{}' has no domain", decl.name())
                });
                symtab.borrow_mut().gensym(&domain)
            };
            dummy_vars_by_decl_id
                .borrow_mut()
                .insert(decl_id, new_dummy.clone());
            new_dummy
        };

        Reference::new(dummy_decl)
    })
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
