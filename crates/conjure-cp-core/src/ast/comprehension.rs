#![allow(clippy::arc_with_non_send_sync)]

use std::{cell::RefCell, collections::BTreeSet, fmt::Display, rc::Rc, sync::atomic::AtomicBool};

use itertools::Itertools as _;
use serde::{Deserialize, Serialize};
use uniplate::{Biplate, Uniplate};

use crate::{ast::Metadata, into_matrix_expr, matrix_expr};

use super::{
    DeclarationPtr, Domain, Expression, Moo, Name, Range, SubModel, SymbolTable,
    ac_operators::ACOperatorKind,
};

// TODO: move this global setting somewhere better?

/// The rewriter to use for rewriting comprehensions.
///
/// True for optimised, false for naive
pub static USE_OPTIMISED_REWRITER_FOR_COMPREHENSIONS: AtomicBool = AtomicBool::new(false);

// TODO: do not use Names to compare variables, use DeclarationPtr and ids instead
// see issue #930
//
// this will simplify *a lot* of the knarly stuff here, but can only be done once everything else
// uses DeclarationPtr.
//
// ~ nikdewally, 10/06/25

/// A comprehension.
#[derive(Clone, PartialEq, Eq, Uniplate, Serialize, Deserialize, Debug)]
#[biplate(to=SubModel)]
#[biplate(to=Expression)]
#[non_exhaustive]
pub struct Comprehension {
    #[doc(hidden)]
    pub return_expression_submodel: SubModel,
    #[doc(hidden)]
    pub generator_submodel: SubModel,
    #[doc(hidden)]
    pub induction_vars: Vec<Name>,
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

    pub fn return_expression(self) -> Expression {
        self.return_expression_submodel.into_single_expression()
    }

    pub fn replace_return_expression(&mut self, new_expr: Expression) {
        let new_expr = match new_expr {
            Expression::And(_, exprs) if (*exprs).clone().unwrap_list().is_some() => {
                Expression::Root(Metadata::new(), (*exprs).clone().unwrap_list().unwrap())
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
                let domain: Domain = decl.domain().unwrap();
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
        let domain = declaration.domain().unwrap();
        assert!(!self.induction_variables.contains(&name));

        self.induction_variables.insert(name.clone());

        // insert into generator symbol table as a variable
        (*self.generator_symboltable)
            .borrow_mut()
            .insert(declaration);

        // insert into return expression symbol table as a given
        (*self.return_expr_symboltable)
            .borrow_mut()
            .insert(DeclarationPtr::new_given(name, domain));

        self
    }

    /// Creates a comprehension with the given return expression.
    ///
    /// If this comprehension is inside an AC-operator, the kind of this operator should be passed
    /// in the `comprehension_kind` field.
    ///
    /// If a comprehension kind is not given, comprehension guards containing decision variables
    /// are invalid, and will cause a panic.
    pub fn with_return_value(
        self,
        mut expression: Expression,
        comprehension_kind: Option<ACOperatorKind>,
    ) -> Comprehension {
        let parent_symboltable = self
            .generator_symboltable
            .as_ref()
            .borrow_mut()
            .parent_mut_unchecked()
            .clone()
            .unwrap();
        let mut generator_submodel = SubModel::new(parent_symboltable.clone());
        let mut return_expression_submodel = SubModel::new(parent_symboltable);

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
                xs => Expression::And(Metadata::new(), Moo::new(into_matrix_expr!(xs.to_vec()))),
            };

            expression = match comprehension_kind {
                ACOperatorKind::And => {
                    Expression::Imply(Metadata::new(), Moo::new(guard_expr), Moo::new(expression))
                }
                ACOperatorKind::Or => Expression::And(
                    Metadata::new(),
                    Moo::new(Expression::And(
                        Metadata::new(),
                        Moo::new(matrix_expr![guard_expr, expression]),
                    )),
                ),

                ACOperatorKind::Sum => {
                    panic!("guards that reference decision variables not yet implemented for sum");
                }

                ACOperatorKind::Product => {
                    panic!(
                        "guards that reference decision variables not yet implemented for product"
                    );
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
