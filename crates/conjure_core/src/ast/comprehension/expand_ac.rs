//! Dummy variable substitution algorithm for `Comprehension::expand_ac`

use std::collections::VecDeque;

use uniplate::{Biplate, Uniplate as _, zipper::Zipper};

use crate::{
    ast::{
        Atom, DeclarationPtr, Expression, Name, ReturnType, SubModel, SymbolTable, Typeable as _,
        ac_operators::ACOperatorKind,
    },
    metadata::Metadata,
};

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
pub(super) fn add_return_expression_to_generator_model(
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
        Box::new(Expression::Atomic(
            Metadata::new(),
            ac_operator.identity().into(),
        )),
        Box::new(zipper.rebuild_root()),
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
pub fn partition_variables(
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

    // // TODO: do we actually need to filter out non decision variables here?
    // let induction_vars = induction_vars
    //     .into_iter()
    //     .filter(|x| symtab.lookup_local(x).unwrap().category_of() >= Category::Decision)
    //     .collect();

    (non_induction_vars, induction_vars)
}

/// Returns `true` if `expr` can be turned into a dummy variable.
#[inline]
pub(super) fn can_be_dummy_variable(expr: &Expression, dummy_variable_type: &ReturnType) -> bool {
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
pub(super) fn contains_non_induction_variables(expr: &Expression, symtab: &SymbolTable) -> bool {
    let names_referenced: VecDeque<Name> = expr.universe_bi();
    // a name is a non-induction variable if its definition is not in the local scope of the
    // comprehension's generators.
    names_referenced
        .iter()
        .any(|x| symtab.lookup_local(x).is_none())
}
