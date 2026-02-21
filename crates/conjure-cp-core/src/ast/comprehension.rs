#![allow(clippy::arc_with_non_send_sync)]

use std::{
    collections::BTreeSet,
    fmt::Display,
    sync::atomic::{AtomicBool, AtomicU8, Ordering},
};

use crate::{ast::Metadata, into_matrix_expr, matrix_expr};
use conjure_cp_core::ast::ReturnType;
use itertools::Itertools as _;
use serde::{Deserialize, Serialize};
use uniplate::{Biplate, Uniplate};

pub use super::quantified_expander::QuantifiedExpander;
use super::{
    DeclarationPtr, Domain, DomainPtr, Expression, Moo, Name, Range, SubModel, SymbolTablePtr,
    Typeable, ac_operators::ACOperatorKind,
};

// TODO: move this global setting somewhere better?

/// The rewriter to use for rewriting comprehensions.
///
/// True for optimised, false for naive
pub static USE_OPTIMISED_REWRITER_FOR_COMPREHENSIONS: AtomicBool = AtomicBool::new(false);

/// Global setting for which comprehension quantified-variable expander to use.
///
/// Defaults to [`QuantifiedExpander::ExpandNative`].
pub static QUANTIFIED_EXPANDER_FOR_COMPREHENSIONS: AtomicU8 =
    AtomicU8::new(QuantifiedExpander::ExpandNative.as_u8());

pub fn set_quantified_expander_for_comprehensions(expander: QuantifiedExpander) {
    QUANTIFIED_EXPANDER_FOR_COMPREHENSIONS.store(expander.as_u8(), Ordering::Relaxed);
}

pub fn quantified_expander_for_comprehensions() -> QuantifiedExpander {
    QuantifiedExpander::from_u8(QUANTIFIED_EXPANDER_FOR_COMPREHENSIONS.load(Ordering::Relaxed))
}

// TODO: do not use Names to compare variables, use DeclarationPtr and ids instead
// see issue #930
//
// this will simplify *a lot* of the knarly stuff here, but can only be done once everything else
// uses DeclarationPtr.
//
// ~ nikdewally, 10/06/25

/// A comprehension.
#[derive(Clone, PartialEq, Eq, Hash, Uniplate, Serialize, Deserialize, Debug)]
#[biplate(to=SubModel)]
#[biplate(to=Expression)]
#[non_exhaustive]
pub struct Comprehension {
    #[doc(hidden)]
    pub return_expression_submodel: SubModel,
    #[doc(hidden)]
    pub generator_submodel: SubModel,
    #[doc(hidden)]
    pub quantified_vars: Vec<Name>,
}

impl Comprehension {
    pub fn domain_of(&self) -> Option<DomainPtr> {
        let return_expr_domain = self
            .return_expression_submodel
            .clone()
            .into_single_expression()
            .domain_of()?;

        // return a list (matrix with index domain int(1..)) of return_expr elements
        Some(Domain::matrix(
            return_expr_domain,
            vec![Domain::int(vec![Range::UnboundedR(1)])],
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

    /// Adds a guard to the comprehension. Returns false if the guard does not only reference quantified variables.
    pub fn add_quantified_guard(&mut self, guard: Expression) -> bool {
        if self.is_quantified_guard(&guard) {
            self.generator_submodel.add_constraint(guard);
            true
        } else {
            false
        }
    }

    /// True iff expr only references quantified variables.
    pub fn is_quantified_guard(&self, expr: &Expression) -> bool {
        is_quantified_guard(&(self.quantified_vars.clone().into_iter().collect()), expr)
    }
}

impl Typeable for Comprehension {
    fn return_type(&self) -> ReturnType {
        self.return_expression_submodel
            .clone()
            .into_single_expression()
            .return_type()
    }
}

impl Display for Comprehension {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        let return_expression = self
            .return_expression_submodel
            .clone()
            .into_single_expression();

        let generator_symbols = self.generator_submodel.symbols().clone();
        let generators = self
            .quantified_vars
            .iter()
            .map(|name| {
                let decl: DeclarationPtr = generator_symbols
                    .lookup_local(name)
                    .expect("quantified variable should be in the generator symbol table");
                let domain: DomainPtr = decl.domain().unwrap();
                format!("{name} : {domain}")
            })
            .collect_vec();

        let guards = self
            .generator_submodel
            .constraints()
            .iter()
            .map(|x| format!("{x}"))
            .collect_vec();

        let generators_and_guards = generators.into_iter().chain(guards).join(", ");

        write!(f, "[ {return_expression} | {generators_and_guards} ]")
    }
}

/// A builder for a comprehension.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct ComprehensionBuilder {
    guards: Vec<Expression>,
    // symbol table containing all the generators
    // for now, this is just used during parsing - a new symbol table is created using this when we initialise the comprehension
    // this is not ideal, but i am chucking all this code very soon anyways...
    generator_symboltable: SymbolTablePtr,
    return_expr_symboltable: SymbolTablePtr,
    quantified_variables: BTreeSet<Name>,
}

impl ComprehensionBuilder {
    pub fn new(symbol_table_ptr: SymbolTablePtr) -> Self {
        ComprehensionBuilder {
            guards: vec![],
            generator_symboltable: SymbolTablePtr::with_parent(symbol_table_ptr.clone()),
            return_expr_symboltable: SymbolTablePtr::with_parent(symbol_table_ptr),
            quantified_variables: BTreeSet::new(),
        }
    }

    /// The symbol table for the comprehension generators
    pub fn generator_symboltable(&mut self) -> SymbolTablePtr {
        self.generator_symboltable.clone()
    }

    /// The symbol table for the comprehension return expression
    pub fn return_expr_symboltable(&mut self) -> SymbolTablePtr {
        self.return_expr_symboltable.clone()
    }

    pub fn guard(mut self, guard: Expression) -> Self {
        self.guards.push(guard);
        self
    }

    pub fn generator(mut self, declaration: DeclarationPtr) -> Self {
        let name = declaration.name().clone();
        let domain = declaration.domain().unwrap();
        assert!(!self.quantified_variables.contains(&name));

        self.quantified_variables.insert(name.clone());

        // insert into generator symbol table as a local quantified variable
        let quantified_decl = DeclarationPtr::new_quantified(name, domain);
        self.generator_symboltable
            .write()
            .insert(quantified_decl.clone());

        // insert into return expression symbol table as a quantified variable
        self.return_expr_symboltable.write().insert(
            DeclarationPtr::new_quantified_from_generator(&quantified_decl)
                .expect("quantified variables should always have a domain"),
        );

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
        let parent_symboltable = self.generator_symboltable.read().parent().clone().unwrap();

        let mut generator_submodel = SubModel::new(parent_symboltable.clone());
        let mut return_expression_submodel = SubModel::new(parent_symboltable);

        *generator_submodel.symbols_ptr_unchecked_mut() = self.generator_symboltable;
        *return_expression_submodel.symbols_ptr_unchecked_mut() = self.return_expr_symboltable;

        // TODO:also allow guards that reference lettings and givens.

        let quantified_variables = self.quantified_variables;

        // only guards referencing quantified variables can go inside the comprehension
        let (mut quantified_guards, mut other_guards): (Vec<_>, Vec<_>) = self
            .guards
            .into_iter()
            .partition(|x| is_quantified_guard(&quantified_variables, x));

        let quantified_variables_2 = quantified_variables.clone();
        let generator_symboltable_ptr = generator_submodel.symbols_ptr_unchecked().clone();

        // fix quantified guard pointers so that they all point to variables in the generator model
        quantified_guards =
            Biplate::<DeclarationPtr>::transform_bi(&quantified_guards, &move |decl| {
                if quantified_variables_2.contains(&decl.name()) {
                    generator_symboltable_ptr
                        .read()
                        .lookup_local(&decl.name())
                        .unwrap()
                } else {
                    decl
                }
            })
            .into_iter()
            .collect_vec();

        let quantified_variables_2 = quantified_variables.clone();
        let return_expr_symboltable_ptr =
            return_expression_submodel.symbols_ptr_unchecked().clone();

        // fix other guard pointers so that they all point to variables in the return expr model
        other_guards = Biplate::<DeclarationPtr>::transform_bi(&other_guards, &move |decl| {
            if quantified_variables_2.contains(&decl.name()) {
                return_expr_symboltable_ptr
                    .read()
                    .lookup_local(&decl.name())
                    .unwrap()
            } else {
                decl
            }
        })
        .into_iter()
        .collect_vec();

        // handle guards that reference non-quantified variables
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

        generator_submodel.add_constraints(quantified_guards);

        return_expression_submodel.add_constraint(expression);

        Comprehension {
            return_expression_submodel,
            generator_submodel,
            quantified_vars: quantified_variables.into_iter().collect_vec(),
        }
    }
}

/// True iff the guard only references quantified variables.
fn is_quantified_guard(quantified_variables: &BTreeSet<Name>, guard: &Expression) -> bool {
    guard
        .universe_bi()
        .iter()
        .all(|x| quantified_variables.contains(x))
}
