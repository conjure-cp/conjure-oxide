use crate::common::parse::json::parse_json;
use std::collections::HashMap;

#[derive(Debug)]
pub struct Model {
    pub variables: HashMap<Name, DecisionVariable>,
    pub constraints: Vec<Expression>,
}

impl Model {
    pub fn new() -> Self {
        Model {
            variables: HashMap::new(),
            constraints: Vec::new(),
        }
    }

    pub fn from_json(s: &String) -> Result<Model, String> {
        parse_json(s)
    }

    // Function to update a DecisionVariable based on its Name
    pub fn update_domain(&mut self, name: &Name, new_domain: Domain) {
        if let Some(decision_var) = self.variables.get_mut(name) {
            decision_var.domain = new_domain;
        }
    }
}

#[derive(Clone, Debug, Eq, PartialEq, Hash)]
pub enum Name {
    UserName(String),
    MachineName(i32),
}

#[derive(Debug, PartialEq)]
pub struct DecisionVariable {
    pub domain: Domain,
}

#[derive(Clone, Debug, PartialEq)]
pub enum Domain {
    BoolDomain,
    IntDomain(Vec<Range<i32>>),
}

#[derive(Clone, Debug, PartialEq)]
pub enum Range<A> {
    Single(A),
    Bounded(A, A),
}

#[derive(Clone, Debug, PartialEq)]
pub enum Expression {
    ConstantInt(i32),
    Reference(Name),
    Sum(Vec<Expression>),
    Eq(Box<Expression>, Box<Expression>),
    Geq(Box<Expression>, Box<Expression>),
}
