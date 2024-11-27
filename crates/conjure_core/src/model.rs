use std::cell::RefCell;
use std::collections::HashSet;
use std::fmt::Debug;
use std::sync::{Arc, RwLock};

use derivative::Derivative;
use serde::{Deserialize, Serialize};
use serde_with::serde_as;

use crate::ast::{DecisionVariable, Domain, Expression, Name, SymbolTable};
use crate::context::Context;
use crate::metadata::Metadata;

/// Represents a computational model containing variables, constraints, and a shared context.
///
/// The `Model` struct holds a set of variables and constraints for manipulating and evaluating symbolic expressions.
///
/// # Fields
/// - `variables`:
///   - Type: `SymbolTable`
///   - A table that links each variable's name to its corresponding `DecisionVariable`.
///   - For example, the name `x` might be linked to a `DecisionVariable` that says `x` can only take values between 1 and 10.
///
/// - `constraints`:
///   - Type: `Expression`
///   - Represents the logical constraints applied to the model's variables.
///   - Can be a single constraint or a combination of various expressions, such as logical operations (e.g., `AND`, `OR`),
///     arithmetic operations (e.g., `SafeDiv`, `UnsafeDiv`), or specialized constraints like `SumEq`.
///
/// - `context`:
///   - Type: `Arc<RwLock<Context<'static>>>`
///   - A shared object that stores global settings and state for the model.
///   - Can be safely read or changed by multiple parts of the program at the same time, making it good for multi-threaded use.
///
/// - `next_var`:
///   - Type: `RefCell<i32>`
///   - A counter used to create new, unique variable names.
///   - Allows updating the counter inside the model without making the whole model mutable.
///
/// # Usage
/// This struct is typically used to:
/// - Define a set of variables and constraints for rule-based evaluation.
/// - Have transformations, optimizations, and simplifications applied to it using a set of rules.
#[serde_as]
#[derive(Derivative, Clone, Debug, Serialize, Deserialize)]
#[derivative(PartialEq, Eq)]
pub struct Model {
    #[serde_as(as = "Vec<(_, _)>")]
    pub variables: SymbolTable,
    pub constraints: Expression,
    #[serde(skip)]
    #[derivative(PartialEq = "ignore")]
    pub context: Arc<RwLock<Context<'static>>>,
    next_var: RefCell<i32>,
}

impl Model {
    pub fn new(
        variables: SymbolTable,
        constraints: Expression,
        context: Arc<RwLock<Context<'static>>>,
    ) -> Model {
        Model {
            variables,
            constraints,
            context,
            next_var: RefCell::new(0),
        }
    }

    pub fn new_empty(context: Arc<RwLock<Context<'static>>>) -> Model {
        Model::new(
            Default::default(),
            Expression::And(Metadata::new(), Vec::new()),
            context,
        )
    }
    // Function to update a DecisionVariable based on its Name
    pub fn update_domain(&mut self, name: &Name, new_domain: Domain) {
        if let Some(decision_var) = self.variables.get_mut(name) {
            decision_var.domain = new_domain;
        }
    }

    pub fn get_domain(&self, name: &Name) -> Option<&Domain> {
        self.variables.get(name).map(|v| &v.domain)
    }

    // Function to add a new DecisionVariable to the Model
    pub fn add_variable(&mut self, name: Name, decision_var: DecisionVariable) {
        self.variables.insert(name, decision_var);
    }

    pub fn get_constraints_vec(&self) -> Vec<Expression> {
        match &self.constraints {
            Expression::And(_, constraints) => constraints.clone(),
            _ => vec![self.constraints.clone()],
        }
    }

    pub fn set_constraints(&mut self, constraints: Vec<Expression>) {
        if constraints.is_empty() {
            self.constraints = Expression::And(Metadata::new(), Vec::new());
        } else if constraints.len() == 1 {
            self.constraints = constraints[0].clone();
        } else {
            self.constraints = Expression::And(Metadata::new(), constraints);
        }
    }

    pub fn set_context(&mut self, context: Arc<RwLock<Context<'static>>>) {
        self.context = context;
    }

    pub fn add_constraint(&mut self, expression: Expression) {
        // ToDo (gs248) - there is no checking whatsoever
        // We need to properly validate the expression but this is just for testing
        let mut constraints = self.get_constraints_vec();
        constraints.push(expression);
        self.set_constraints(constraints);
    }

    pub fn add_constraints(&mut self, expressions: Vec<Expression>) {
        let mut constraints = self.get_constraints_vec();
        constraints.extend(expressions);
        self.set_constraints(constraints);
    }

    /// Returns an arbitrary variable name that is not in the model.
    pub fn gensym(&self) -> Name {
        let num = *self.next_var.borrow();
        *(self.next_var.borrow_mut()) += 1;
        Name::MachineName(num) // incremented when inserted
    }

    /// Extends the models symbol table with the given symbol table, updating the gensym counter if
    /// necessary.
    ///
    pub fn extend_sym_table(&mut self, symbol_table: SymbolTable) {
        if symbol_table.keys().len() > self.variables.keys().len() {
            let new_vars = symbol_table.keys().collect::<HashSet<_>>();
            let old_vars = self.variables.keys().collect::<HashSet<_>>();

            for added_var in new_vars.difference(&old_vars) {
                let mut next_var = self.next_var.borrow_mut();
                match *added_var {
                    Name::UserName(_) => {}
                    Name::MachineName(m) => {
                        if *m >= *next_var {
                            *next_var = *m + 1;
                        }
                    }
                }
            }
        }
        self.variables.extend(symbol_table);
    }
}
