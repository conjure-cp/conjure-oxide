use std::{
    collections::{HashMap, VecDeque},
    rc::Rc,
    sync::{Arc, Mutex, RwLock, atomic::Ordering},
};

use conjure_cp::{
    ast::{
        Atom, DeclarationKind, DeclarationPtr, Expression, Metadata, Model, Moo, Name, ReturnType,
        SubModel, SymbolTable, Typeable as _,
        ac_operators::ACOperatorKind,
        comprehension::{Comprehension, USE_OPTIMISED_REWRITER_FOR_COMPREHENSIONS},
        serde::{HasId as _, ObjId},
    },
    bug,
    context::Context,
    rule_engine::{resolve_rule_sets, rewrite_morph, rewrite_naive},
    solver::{Solver, SolverError, SolverFamily, adaptors::Minion},
};
use tracing::warn;
use uniplate::{Biplate, Uniplate as _, zipper::Zipper};

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
pub fn expand_ac(
    comprehension: Comprehension,
    symtab: &mut SymbolTable,
    ac_operator: ACOperatorKind,
) -> Result<Vec<Expression>, SolverError> {
    // ADD RETURN EXPRESSION TO GENERATOR MODEL AS CONSTRAINT
    // ======================================================

    // References to induction variables in the return expression point to entries in the
    // return_expression symbol table.
    //
    // Change these to point to the corresponding entry in the generator symbol table instead.
    //
    // In the generator symbol-table, induction variables are decision variables (as we are
    // solving for them), but in the return expression symbol table they are givens.
    let induction_vars_2 = comprehension.induction_vars.clone();
    let generator_symtab_ptr = Rc::clone(comprehension.generator_submodel.symbols_ptr_unchecked());
    let return_expression =
        comprehension
            .clone()
            .return_expression()
            .transform_bi(&move |decl: DeclarationPtr| {
                // if this variable is an induction var...
                if induction_vars_2.contains(&decl.name()) {
                    // ... use the generator symbol tables version of it

                    (*generator_symtab_ptr)
                        .borrow()
                        .lookup_local(&decl.name())
                        .unwrap()
                } else {
                    decl
                }
            });

    // Replace all boolean expressions referencing non-induction variables in the return
    // expression with dummy variables. This allows us to add it as a constraint to the
    // generator model.
    let generator_submodel = add_return_expression_to_generator_model(
        comprehension.generator_submodel.clone(),
        return_expression,
        &ac_operator,
    );

    // REWRITE GENERATOR MODEL AND PASS TO MINION
    // ==========================================

    let mut generator_model = Model::new(Arc::new(RwLock::new(Context::default())));

    *generator_model.as_submodel_mut() = generator_submodel;

    // only branch on the induction variables.
    generator_model.search_order = Some(comprehension.induction_vars.clone());

    let extra_rule_sets = &[
        "Base",
        "Constant",
        "Bubble",
        "Better_AC_Comprehension_Expansion",
    ];

    let rule_sets = resolve_rule_sets(SolverFamily::Minion, extra_rule_sets).unwrap();

    let generator_model = if USE_OPTIMISED_REWRITER_FOR_COMPREHENSIONS.load(Ordering::Relaxed) {
        rewrite_morph(generator_model, &rule_sets, false)
    } else {
        rewrite_naive(&generator_model, &rule_sets, false, false).unwrap()
    };

    let minion = Solver::new(Minion::new());
    let minion = minion.load_model(generator_model.clone());

    let minion = match minion {
        Err(e) => {
            warn!(why=%e,model=%generator_model,"Loading generator model failed, failing expand_ac rule");
            return Err(e);
        }
        Ok(minion) => minion,
    };

    // REWRITE RETURN EXPRESSION
    // =========================

    let return_expression_submodel = comprehension.return_expression_submodel.clone();
    let mut return_expression_model = Model::new(Arc::new(RwLock::new(Context::default())));
    *return_expression_model.as_submodel_mut() = return_expression_submodel;

    let return_expression_model =
        if USE_OPTIMISED_REWRITER_FOR_COMPREHENSIONS.load(Ordering::Relaxed) {
            rewrite_morph(return_expression_model, &rule_sets, false)
        } else {
            rewrite_naive(&return_expression_model, &rule_sets, false, false).unwrap()
        };

    let values = Arc::new(Mutex::new(Vec::new()));
    let values_ptr = Arc::clone(&values);

    // SOLVE FOR THE INDUCTION VARIABLES, AND SUBSTITUTE INTO THE REWRITTEN RETURN EXPRESSION
    // ======================================================================================

    tracing::debug!(model=%generator_model,comprehension=%comprehension,"Minion solving comprehnesion (ac mode)");

    minion.solve(Box::new(move |sols| {
        // TODO: deal with represented names if induction variables are abslits.
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

        // we only want to substitute induction variables.
        // (definitely not machine names, as they mean something different in this scope!)
        let value: HashMap<_, _> = value
            .into_iter()
            .filter(|(n, _)| comprehension.induction_vars.contains(n))
            .collect();

        let value_ptr = Arc::new(value);
        let value_ptr_2 = Arc::clone(&value_ptr);

        // substitute in the values for the induction variables
        let return_expression = return_expression.transform_bi(&move |x: Atom| {
            let Atom::Reference(ref ptr) = x else {
                return x;
            };

            // is this referencing an induction var?
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
            // do not add givens for induction vars to the parent symbol table.
            if value_ptr.get(&name).is_some()
                && matches!(&decl.kind() as &DeclarationKind, DeclarationKind::Given(_))
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
                Atom::Reference(new_decl.clone())
            } else {
                atom
            }
        });

        return_expressions.push(return_expression);
    }

    Ok(return_expressions)
}

/// Eliminate all references to non induction variables by introducing dummy variables to the
/// return expression. This modified return expression is added to the generator model, which is
/// returned.
///
/// Dummy variables must be the same type as the AC operators identity value.
///
/// To reduce the number of dummy variables, we turn the largest expression containing only
/// non induction variables and of the correct type into a dummy variable.
///
/// If there is no such expression, (e.g. and[(a<i) | i: int(1..10)]) , we use the smallest
/// expression of the correct type that contains a non induction variable. This ensures that
/// we lose as few references to induction variables as possible.
fn add_return_expression_to_generator_model(
    mut generator_submodel: SubModel,
    return_expression: Expression,
    ac_operator: &ACOperatorKind,
) -> SubModel {
    let mut zipper = Zipper::new(return_expression);
    let mut symtab = generator_submodel.symbols_mut();

    // for sum/product we want to put integer expressions into dummy variables,
    // for and/or we want to put boolean expressions into dummy variables.
    let dummy_var_type = ac_operator
        .identity()
        .return_type()
        .expect("identity value of an ACOpKind should always have a ReturnType");

    'outer: loop {
        let focus: &mut Expression = zipper.focus_mut();

        let (non_induction_vars, induction_vars) = partition_variables(focus, &symtab);

        // an expression or its descendants needs to be turned into a dummy variable if it
        // contains non-induction variables.
        let has_non_induction_vars = !non_induction_vars.is_empty();

        // does this expression contain induction variables?
        let has_induction_vars = !induction_vars.is_empty();

        // can this expression be turned into a dummy variable?
        let can_be_dummy_var = can_be_dummy_variable(focus, &dummy_var_type);

        // The expression and its descendants don't need a dummy variables, so we don't
        // need to descend into its children.
        if !has_non_induction_vars {
            // go to next node or quit
            while zipper.go_right().is_none() {
                let Some(()) = zipper.go_up() else {
                    // visited all nodes
                    break 'outer;
                };
            }
            continue;
        }

        // The expression contains non-induction variables:

        // does this expression have any children that can be turned into dummy variables?
        let has_eligible_child = focus.universe().iter().skip(1).any(|expr| {
            // eligible if it can be turned into a dummy variable, and turning it into a
            // dummy variable removes a non-induction variable from the model.
            can_be_dummy_variable(expr, &dummy_var_type)
                && contains_non_induction_variables(expr, &symtab)
        });

        // This expression has no child that can be turned into a dummy variable, but can
        // be a dummy variable => turn it into a dummy variable and continue.
        if !has_eligible_child && can_be_dummy_var {
            // introduce dummy var and continue
            let dummy_domain = focus.domain_of().unwrap();
            let dummy_decl = symtab.gensym(&dummy_domain);
            *focus = Expression::Atomic(Metadata::new(), Atom::Reference(dummy_decl));

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
                    *focus = Expression::Atomic(Metadata::new(), Atom::Reference(dummy_decl));

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
        // If the expression contains induction variables as well as non-induction
        // variables, try to retain the induction varables by finding a child that can be
        // made a dummy variable which has only non-induction variables.
        else if has_eligible_child && has_induction_vars {
            zipper
                .go_down()
                .expect("we know the focus has a child, so zipper.go_down() should succeed");
        }
        // This expression contains no induction variables, so no point trying to turn a
        // child into a dummy variable.
        else if has_eligible_child && !has_induction_vars {
            // introduce dummy var and continue
            let dummy_domain = focus.domain_of().unwrap();
            let dummy_decl = symtab.gensym(&dummy_domain);
            *focus = Expression::Atomic(Metadata::new(), Atom::Reference(dummy_decl));

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

    // double check that the above transformation didn't miss any stray non induction vars
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

/// Returns a tuple of non-induction decision variables and induction variables inside the expression.
///
/// As lettings, givens, etc. will eventually be subsituted for constants, this only returns
/// non-induction _decision_ variables.
#[inline]
fn partition_variables(
    expr: &Expression,
    symtab: &SymbolTable,
) -> (VecDeque<Name>, VecDeque<Name>) {
    // doing this as two functions non_induction_variables and induction_variables might've been
    // easier to read.
    //
    // However, doing this in one function avoids an extra universe call...
    let (non_induction_vars, induction_vars): (VecDeque<Name>, VecDeque<Name>) =
        Biplate::<Name>::universe_bi(expr)
            .into_iter()
            .partition(|x| symtab.lookup_local(x).is_none());

    (non_induction_vars, induction_vars)
}

/// Returns `true` if `expr` can be turned into a dummy variable.
#[inline]
fn can_be_dummy_variable(expr: &Expression, dummy_variable_type: &ReturnType) -> bool {
    // do not put root expression in a dummy variable or things go wrong.
    if matches!(expr, Expression::Root(_, _)) {
        return false;
    };

    // is the expression the same type as the dummy variable?
    expr.return_type()
        .is_some_and(|x| x == *dummy_variable_type)
}

/// Returns `true` if `expr` or its descendants contains non-induction variables.
#[inline]
fn contains_non_induction_variables(expr: &Expression, symtab: &SymbolTable) -> bool {
    let names_referenced: VecDeque<Name> = expr.universe_bi();
    // a name is a non-induction variable if its definition is not in the local scope of the
    // comprehension's generators.
    names_referenced
        .iter()
        .any(|x| symtab.lookup_local(x).is_none())
}
