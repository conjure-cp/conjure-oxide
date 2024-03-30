use std::cell::RefCell;
use std::fmt::Debug;
use std::sync::{Arc, RwLock};

use derivative::Derivative;
use serde::{Deserialize, Serialize};
use serde_with::serde_as;

use crate::ast::{DecisionVariable, Domain, Expression, Name, SymbolTable};
use crate::context::Context;
use crate::metadata::Metadata;

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
        Model::new(Default::default(), Expression::Nothing, context)
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
            Expression::Nothing => vec![],
            _ => vec![self.constraints.clone()],
        }
    }

    pub fn set_constraints(&mut self, constraints: Vec<Expression>) {
        if constraints.is_empty() {
            self.constraints = Expression::Nothing;
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
}
