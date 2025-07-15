#![allow(clippy::arc_with_non_send_sync)]
use std::{
    cell::RefCell,
    collections::{BTreeSet, HashMap, VecDeque},
    fmt::Display,
    rc::Rc,
    sync::{Arc, Mutex, RwLock},
};

use itertools::Itertools as _;
use serde::{Deserialize, Serialize};
use tracing::warn;
use uniplate::{Biplate, Uniplate, zipper::Zipper};

use crate::{
    ast::{
        Atom, DeclarationKind, Typeable as _,
        serde::{HasId as _, ObjId},
    },
    bug,
    context::Context,
    into_matrix_expr, matrix_expr,
    metadata::Metadata,
    rule_engine::{resolve_rule_sets, rewrite_naive},
    solver::{Solver, SolverError},
};

use super::{
    DeclarationPtr, Domain, Expression, Model, Name, Range, SubModel, SymbolTable,
    ac_operators::ACOperatorKind,
};

// TODO: do not use Names to compare variables, use DeclarationPtr and ids instead
// see issue #930
//
// this will simplify *a lot* of the knarly stuff here, but can only be done once everything else
// uses DeclarationPtr.
//
// ~ nikdewally, 10/06/25

pub enum ComprehensionKind {
    Sum,
    And,
    Or,
}
/// A comprehension.
#[derive(Clone, PartialEq, Eq, Uniplate, Serialize, Deserialize, Debug)]
#[biplate(to=SubModel)]
#[biplate(to=Expression)]
pub struct Comprehension {
    return_expression_submodel: SubModel,
    generator_submodel: SubModel,
    induction_vars: Vec<Name>,
}

impl Comprehension {
    pub fn domain_of(&self) -> Option<Domain> {
        let return_expr_domain = self
            .return_expression_submodel
            .clone()
            .into_single_expression()
            .domain_of()?;

        // return a list (matrix with index domain int(1..)) of return_expr elements
        Some(Domain::Matrix(
            Box::new(return_expr_domain),
            vec![Domain::Int(vec![Range::UnboundedR(1)])],
        ))
    }

    /// Expands the comprehension using Minion, returning the resulting expressions.
    ///
    /// This method performs simple pruning of the induction variables: an expression is returned
    /// for each assignment to the induction variables that satisfy the static guards of the
    /// comprehension. If the comprehension is inside an associative-commutative operation, use
    /// [`expand_ac`] instead, as this performs further pruning of "uninteresting" return values.
    ///
    /// If successful, this modifies the symbol table given to add aux-variables needed inside the
    /// expanded expressions.
    pub fn expand_simple(self, symtab: &mut SymbolTable) -> Result<Vec<Expression>, SolverError> {
        let minion = Solver::new(crate::solver::adaptors::Minion::new());
        // FIXME: weave proper context through
        let mut model = Model::new(Arc::new(RwLock::new(Context::default())));

        // only branch on the induction variables.
        model.search_order = Some(self.induction_vars.clone());
        *model.as_submodel_mut() = self.generator_submodel.clone();

        // TODO:  if expand_ac is enabled, add Better_AC_Comprehension_Expansion here.

        // call rewrite here as well as in expand_ac, just to be consistent
        let extra_rule_sets = &["Base", "Constant", "Bubble"];

        let rule_sets =
            resolve_rule_sets(crate::solver::SolverFamily::Minion, extra_rule_sets).unwrap();

        let model = crate::rule_engine::rewrite_naive(&model, &rule_sets, false).unwrap();

        // HACK: also call the rewriter to rewrite inside the comprehension
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

        let return_expression_submodel = self.return_expression_submodel.clone();
        let mut return_expression_model = Model::new(Arc::new(RwLock::new(Context::default())));
        *return_expression_model.as_submodel_mut() = return_expression_submodel;
        return_expression_model =
            rewrite_naive(&return_expression_model, &rule_sets, false).unwrap();

        let minion = minion.load_model(model.clone())?;

        let values = Arc::new(Mutex::new(Vec::new()));
        let values_ptr = Arc::clone(&values);

        tracing::debug!(model=%model.clone(),comprehension=%self.clone(),"Minion solving comprehension (simple mode)");
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
                .filter(|(n, _)| self.induction_vars.contains(n))
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

    /// Expands the comprehension using Minion, returning the resulting expressions.
    ///
    /// This method is only suitable for comprehensions inside an AC operator. The AC operator that
    /// contains this comprehension should be passed into the `ac_operator` argument.
    ///
    /// This method performs additional pruning of "uninteresting" values, only possible when the
    /// comprehension is inside an AC operator.
    ///
    /// TODO: more details on what this does....
    ///
    /// If successful, this modifies the symbol table given to add aux-variables needed inside the
    /// expanded expressions.
    pub fn expand_ac(
        self,
        symtab: &mut SymbolTable,
        ac_operator: ACOperatorKind,
    ) -> Result<Vec<Expression>, SolverError> {
        // FIXME: weave proper context through

        let minion = Solver::new(crate::solver::adaptors::Minion::new());

        let mut generator_model = Model::new(Arc::new(RwLock::new(Context::default())));
        *generator_model.as_submodel_mut() = self.generator_submodel.clone();

        // only branch on the induction variables.
        generator_model.search_order = Some(self.induction_vars.clone());

        // Replace all boolean expressions referencing non-induction variables in the return
        // expression with dummy variables.
        //
        // add the modified return expression, and these dummy variables, to the generator model.

        // the bottom up version

        let generator_symtab_ptr = Rc::clone(generator_model.as_submodel().symbols_ptr_unchecked());

        // for sum/product we want to put integer expressions into dummy variables,
        // for and/or we want to put boolean expressions into dummy variables.
        let dummy_var_type = ac_operator
            .identity()
            .return_type()
            .expect("identity value of an ACOpKind should always have a ReturnType");

        // Eliminate all references to non induction variables by introducing dummy variables.
        //
        // Dummy variables must be the same type as the AC operators identity value.
        //
        // To reduce the number of dummy variables, we turn the largest expression containing only
        // non induction variables and of the correct type into a dummy variable.
        //
        // If there is no such expression, (e.g. and[(a<i) | i: int(1..10)]) , we use the smallest
        // expression of the correct type that contains a non induction variable. This ensures that
        // we lose as few references to induction variables as possible.
        let generator_symtab_ptr_2 = Rc::clone(&generator_symtab_ptr);
        let induction_vars_2 = self.induction_vars.clone();
        let return_expression =
            self.clone()
                .return_expression()
                .transform_bi(&move |decl: DeclarationPtr| {
                    // if this variable is an induction var...
                    if induction_vars_2.contains(&decl.name()) {
                        // ... use the generator symbol tables version of it

                        (*generator_symtab_ptr_2)
                            .borrow()
                            .lookup_local(&decl.name())
                            .unwrap()
                    } else {
                        decl
                    }
                });
        let mut zipper = Zipper::new(return_expression);

        // References to induction variables in the return expression point to entries in the
        // return_expression symbol table.
        //
        // Change these to point to the corresponding entry in the generator symbol table instead.
        //
        // In the generator symbol-table, induction variables are decision variables (as we are
        // solving for them), but in the return expression symbol table they are givens.
        {}

        {
            let mut generator_symtab = generator_symtab_ptr.borrow_mut();
            'outer: loop {
                let focus: &mut Expression = zipper.focus_mut();
                let names_referenced: VecDeque<Name> = focus.universe_bi();
                let has_non_induction_vars = names_referenced
                    .iter()
                    .any(|x| generator_symtab.lookup_local(x).is_none());

                // dont care about lettings, as they will be substituted
                let has_induction_vars = names_referenced.iter().any(|x| {
                    generator_symtab.lookup_local(x).is_some_and(|x| {
                        !matches!(
                            &x.kind() as &DeclarationKind,
                            DeclarationKind::ValueLetting(_) | DeclarationKind::DomainLetting(_)
                        )
                    })
                });

                // cannot remove root expression or things go wrong
                let is_right_type = focus.return_type().is_some_and(|x| x == dummy_var_type)
                    && !matches!(focus, Expression::Root(_, _));

                if !has_non_induction_vars {
                    // doesn't need a dummy variable - continue

                    // go to next node or quit
                    while zipper.go_right().is_none() {
                        let Some(()) = zipper.go_up() else {
                            // visited all nodes
                            break 'outer;
                        };
                    }
                } else if !has_induction_vars {
                    // have non-induction vars but no induction vars

                    // introduce a dummy variable if we can, otherwise find a child that can.
                    if is_right_type {
                        // introduce dummy var and continue
                        let dummy_domain = focus.domain_of().unwrap();
                        let dummy_decl = generator_symtab.gensym(&dummy_domain);
                        *focus = Expression::Atomic(Metadata::new(), Atom::Reference(dummy_decl));

                        // go to next node
                        while zipper.go_right().is_none() {
                            let Some(()) = zipper.go_up() else {
                                // visited all nodes
                                break 'outer;
                            };
                        }
                    } else {
                        // skip self
                        let has_eligible_descendant = focus.universe().iter().skip(1).any(|expr| {
                            let names_referenced: VecDeque<Name> = expr.universe_bi();
                            let has_non_induction_vars = names_referenced
                                .iter()
                                .any(|name| generator_symtab.lookup_local(name).is_none());
                            let is_right_type =
                                expr.return_type().is_some_and(|ty| ty == dummy_var_type)
                                    && !matches!(expr, Expression::Root(_, _));
                            is_right_type && has_non_induction_vars
                        });

                        assert!(
                            has_eligible_descendant,
                            "An expression containing an non-induction variable \
                        should either be able to be turned into a dummy variable, or should \
                        have a descendant that can be turned into a dummy variable. (Has traversal \
                        gone too deep and gone into an expression that is turnable into a dummy \
                        variable?"
                        );

                        zipper.go_down().expect(
                            "we know the focus has a child, so zipper.go_down() should succeed",
                        );
                    }
                } else {
                    // have both induction and non induction vars.

                    // Ideally, we want our dummy variables to contain only non-induction variables -
                    // if we have a child that contains non-induction vars and is of the right type,
                    // use that as the dummy variable instead.

                    // skip self
                    let has_eligible_descendant = focus.universe().iter().skip(1).any(|expr| {
                        let names_referenced: VecDeque<Name> = expr.universe_bi();
                        let has_non_induction_vars = names_referenced
                            .iter()
                            .any(|name| generator_symtab.lookup_local(name).is_none());
                        let is_right_type =
                            expr.return_type().is_some_and(|ty| ty == dummy_var_type)
                                && !matches!(expr, Expression::Root(_, _));
                        is_right_type && has_non_induction_vars
                    });

                    if has_eligible_descendant {
                        zipper.go_down().expect(
                            "we know the focus has a child, so zipper.go_down() should succeed",
                        );
                    } else {
                        // no better expression...

                        assert!(
                            is_right_type,
                            "This expression must be put in a dummy variable as \
                            none of its descendants can be.\nTherefore, it should be of the right \
                            type.\n\
                            Expression: {}\n\
                            Expected Type: {:#?}, Actual Type: {:#?} ",
                            focus,
                            dummy_var_type,
                            focus.return_type().unwrap()
                        );

                        // introduce dummy variable
                        let dummy_domain = focus.domain_of().unwrap();
                        let dummy_decl = generator_symtab.gensym(&dummy_domain);
                        *focus = Expression::Atomic(Metadata::new(), Atom::Reference(dummy_decl));

                        // go to next node
                        while zipper.go_right().is_none() {
                            let Some(()) = zipper.go_up() else {
                                // visited all nodes
                                break 'outer;
                            };
                        }
                    }
                }
            }
        }
        let new_return_expr = zipper.rebuild_root();

        // double check that the above transformation didn't miss any stray non induction vars
        assert!(
            Biplate::<DeclarationPtr>::universe_bi(&new_return_expr)
                .iter()
                .all(|x| (*generator_symtab_ptr)
                    .borrow()
                    .lookup_local(&x.name())
                    .is_some()),
            "generator model should only contain references to variables in its symbol table."
        );

        let return_expr_constraint = Expression::Neq(
            Metadata::new(),
            Box::new(Expression::Atomic(
                Metadata::new(),
                ac_operator.identity().into(),
            )),
            Box::new(new_return_expr),
        );

        generator_model
            .as_submodel_mut()
            .add_constraint(return_expr_constraint);

        // rest is same as expand_simple (at time of writing)

        // TODO: move the common code below and in expand_simple into the same function so that we
        // can keep them in sync.

        // FIXME: some less hacky way to rewrite the modified generator model!
        let extra_rule_sets = &[
            "Base",
            "Constant",
            "Bubble",
            "Better_AC_Comprehension_Expansion",
        ];

        let rule_sets =
            resolve_rule_sets(crate::solver::SolverFamily::Minion, extra_rule_sets).unwrap();

        let generator_model =
            crate::rule_engine::rewrite_naive(&generator_model, &rule_sets, false).unwrap();

        // HACK: also call the rewriter to rewrite inside the comprehension
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

        let return_expression_submodel = self.return_expression_submodel.clone();
        let mut return_expression_model = Model::new(Arc::new(RwLock::new(Context::default())));
        *return_expression_model.as_submodel_mut() = return_expression_submodel;
        return_expression_model =
            rewrite_naive(&return_expression_model, &rule_sets, false).unwrap();

        let minion = minion.load_model(generator_model.clone());

        // maybe this is a nested comprehension, so we dont have enough information to rewrite the
        // generator model.
        //
        let minion = match minion {
            Err(e) => {
                warn!(why=%e,model=%generator_model,"Loading generator model failed, failing expand_ac rule");
                return Err(e);
            }
            Ok(minion) => minion,
        };

        let values = Arc::new(Mutex::new(Vec::new()));
        let values_ptr = Arc::clone(&values);

        tracing::debug!(model=%generator_model.clone(),comprehension=%self.clone(),"Minion solving comprehnesion (ac mode)");
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
                .filter(|(n, _)| self.induction_vars.contains(n))
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

    pub fn return_expression(self) -> Expression {
        self.return_expression_submodel.into_single_expression()
    }

    pub fn replace_return_expression(&mut self, new_expr: Expression) {
        let new_expr = match new_expr {
            Expression::And(_, exprs) if exprs.clone().unwrap_list().is_some() => {
                Expression::Root(Metadata::new(), exprs.unwrap_list().unwrap())
            }
            expr => Expression::Root(Metadata::new(), vec![expr]),
        };

        *self.return_expression_submodel.root_mut_unchecked() = new_expr;
    }

    /// Adds a guard to the comprehension. Returns false if the guard does not only reference induction variables.
    pub fn add_induction_guard(&mut self, guard: Expression) -> bool {
        if self.is_induction_guard(&guard) {
            self.generator_submodel.add_constraint(guard);
            true
        } else {
            false
        }
    }

    /// True iff expr only references induction variables.
    pub fn is_induction_guard(&self, expr: &Expression) -> bool {
        is_induction_guard(&(self.induction_vars.clone().into_iter().collect()), expr)
    }
}

impl Display for Comprehension {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        let generators: String = self
            .generator_submodel
            .symbols()
            .clone()
            .into_iter_local()
            .map(|(name, decl): (Name, DeclarationPtr)| {
                let domain: Domain = decl.domain().unwrap().clone();
                (name, domain)
            })
            .map(|(name, domain): (Name, Domain)| format!("{name}: {domain}"))
            .join(",");

        let guards = self
            .generator_submodel
            .constraints()
            .iter()
            .map(|x| format!("{x}"))
            .join(",");

        let generators_and_guards = itertools::join([generators, guards], ",");

        let expression = &self.return_expression_submodel;
        write!(f, "[{expression} | {generators_and_guards}]")
    }
}

/// A builder for a comprehension.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct ComprehensionBuilder {
    guards: Vec<Expression>,
    // symbol table containing all the generators
    // for now, this is just used during parsing - a new symbol table is created using this when we initialise the comprehension
    // this is not ideal, but i am chucking all this code very soon anyways...
    generator_symboltable: Rc<RefCell<SymbolTable>>,
    return_expr_symboltable: Rc<RefCell<SymbolTable>>,
    induction_variables: BTreeSet<Name>,
}

impl ComprehensionBuilder {
    pub fn new(symbol_table_ptr: Rc<RefCell<SymbolTable>>) -> Self {
        ComprehensionBuilder {
            guards: vec![],
            generator_symboltable: Rc::new(RefCell::new(SymbolTable::with_parent(
                symbol_table_ptr.clone(),
            ))),
            return_expr_symboltable: Rc::new(RefCell::new(SymbolTable::with_parent(
                symbol_table_ptr,
            ))),
            induction_variables: BTreeSet::new(),
        }
    }

    /// The symbol table for the comprehension generators
    pub fn generator_symboltable(&mut self) -> Rc<RefCell<SymbolTable>> {
        Rc::clone(&self.generator_symboltable)
    }

    /// The symbol table for the comprehension return expression
    pub fn return_expr_symboltable(&mut self) -> Rc<RefCell<SymbolTable>> {
        Rc::clone(&self.return_expr_symboltable)
    }

    pub fn guard(mut self, guard: Expression) -> Self {
        self.guards.push(guard);
        self
    }

    pub fn generator(mut self, declaration: DeclarationPtr) -> Self {
        let name = declaration.name().clone();
        let domain = declaration.domain().unwrap().clone();
        assert!(!self.induction_variables.contains(&name));

        self.induction_variables.insert(name.clone());

        // insert into generator symbol table as a variable
        (*self.generator_symboltable)
            .borrow_mut()
            .insert(declaration.clone());

        // insert into return expression symbol table as a given
        (*self.return_expr_symboltable)
            .borrow_mut()
            .insert(DeclarationPtr::new_given(name, domain));

        self
    }

    /// Creates a comprehension with the given return expression.
    ///
    /// If a comprehension kind is not given, comprehension guards containing decision variables
    /// are invalid, and will cause a panic.
    pub fn with_return_value(
        self,
        mut expression: Expression,
        comprehension_kind: Option<ComprehensionKind>,
    ) -> Comprehension {
        let parent_symboltable = self
            .generator_symboltable
            .as_ref()
            .borrow_mut()
            .parent_mut_unchecked()
            .clone()
            .unwrap();
        let mut generator_submodel = SubModel::new(parent_symboltable.clone());
        let mut return_expression_submodel = SubModel::new(parent_symboltable.clone());

        *generator_submodel.symbols_ptr_unchecked_mut() = self.generator_symboltable;
        *return_expression_submodel.symbols_ptr_unchecked_mut() = self.return_expr_symboltable;

        // TODO:also allow guards that reference lettings and givens.

        let induction_variables = self.induction_variables;

        // only guards referencing induction variables can go inside the comprehension
        let (mut induction_guards, mut other_guards): (Vec<_>, Vec<_>) = self
            .guards
            .into_iter()
            .partition(|x| is_induction_guard(&induction_variables, x));

        let induction_variables_2 = induction_variables.clone();
        let generator_symboltable_ptr = generator_submodel.symbols_ptr_unchecked().clone();

        // fix induction guard pointers so that they all point to variables in the generator model
        induction_guards =
            Biplate::<DeclarationPtr>::transform_bi(&induction_guards, &move |decl| {
                if induction_variables_2.contains(&decl.name()) {
                    (*generator_symboltable_ptr)
                        .borrow()
                        .lookup_local(&decl.name())
                        .unwrap()
                } else {
                    decl
                }
            })
            .into_iter()
            .collect_vec();

        let induction_variables_2 = induction_variables.clone();
        let return_expr_symboltable_ptr =
            return_expression_submodel.symbols_ptr_unchecked().clone();

        // fix other guard pointers so that they all point to variables in the return expr model
        other_guards = Biplate::<DeclarationPtr>::transform_bi(&other_guards, &move |decl| {
            if induction_variables_2.contains(&decl.name()) {
                (*return_expr_symboltable_ptr)
                    .borrow()
                    .lookup_local(&decl.name())
                    .unwrap()
            } else {
                decl
            }
        })
        .into_iter()
        .collect_vec();

        // handle guards that reference non-induction variables
        if !other_guards.is_empty() {
            let comprehension_kind = comprehension_kind.expect(
                "if any guards reference decision variables, a comprehension kind should be given",
            );

            let guard_expr = match other_guards.as_slice() {
                [x] => x.clone(),
                xs => Expression::And(Metadata::new(), Box::new(into_matrix_expr!(xs.to_vec()))),
            };

            expression = match comprehension_kind {
                ComprehensionKind::And => {
                    Expression::Imply(Metadata::new(), Box::new(guard_expr), Box::new(expression))
                }
                ComprehensionKind::Or => Expression::And(
                    Metadata::new(),
                    Box::new(Expression::And(
                        Metadata::new(),
                        Box::new(matrix_expr![guard_expr, expression]),
                    )),
                ),

                ComprehensionKind::Sum => {
                    panic!("guards that reference decision variables not yet implemented for sum");
                }
            }
        }

        generator_submodel.add_constraints(induction_guards);

        return_expression_submodel.add_constraint(expression);

        Comprehension {
            return_expression_submodel,
            generator_submodel,
            induction_vars: induction_variables.into_iter().collect_vec(),
        }
    }
}

/// True iff the guard only references induction variables.
fn is_induction_guard(induction_variables: &BTreeSet<Name>, guard: &Expression) -> bool {
    guard
        .universe_bi()
        .iter()
        .all(|x| induction_variables.contains(x))
}
