use std::collections::HashMap;
use super::ast::*;

pub struct ModelBuilder {
    pub variables: HashMap<Name, DecisionVariable>,
    pub constraints: Vec<Expression>,
}

impl ModelBuilder {
    pub fn new() -> ModelBuilder {
        ModelBuilder {
            variables: HashMap::new(),
            constraints: Vec::new(),
        }
    }

    pub fn add_constraint(mut self, constraint: Expression) -> Self {
        self.constraints.push(constraint);
        self
    }

    pub fn add_var(mut self, name: Name, domain: Domain) -> Self {
        assert!(self.variables.get(&name).is_none());
        self.variables.insert(
            name,
            DecisionVariable {
                domain,
            },
        );
        self
    }

    pub fn build(self) -> Model {
        Model {
            variables: self.variables,
            constraints: self.constraints,
        }
    }
}
