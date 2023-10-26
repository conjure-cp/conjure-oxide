use std::collections::HashMap;

#[derive(Debug)]
pub struct Model {
    pub variables: HashMap<Name, DecisionVariable>,
    pub constraints: Vec<Expression>,
}

impl Model {
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

#[derive(Debug)]
pub struct DecisionVariable {
    pub domain: Domain,
}

#[derive(Debug)]
pub enum Domain {
    BoolDomain,
    IntDomain(Vec<Range<i32>>),
}

#[derive(Debug)]
pub enum Range<A> {
    Single(A),
    Bounded(A, A),
}

#[derive(Clone, Debug)]
pub enum Expression {
    ConstantInt(i32),
    Reference(Name),
    Sum(Vec<Expression>),
    Eq(Box<Expression>, Box<Expression>),
    Geq(Box<Expression>, Box<Expression>),
}
