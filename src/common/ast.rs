use std::{cell::RefCell, rc::Rc};

#[derive(Debug)]
pub struct Model {
    pub statements: Vec<Statement>,
}

#[derive(Debug)]
pub enum Statement {
    Declaration(Rc<RefCell<DecisionVariable>>),
    Constraint(Expression),
}

#[derive(Debug)]
pub enum Name {
    UserName(String),
    MachineName(i32),
}

#[derive(Debug)]
pub struct DecisionVariable {
    pub name: Name,
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

#[derive(Debug)]
pub enum Expression {
    ConstantInt(i32),
    Reference(Rc<RefCell<DecisionVariable>>),
    Sum(Vec<Expression>),
    Eq(Box<Expression>, Box<Expression>),
    Geq(Box<Expression>, Box<Expression>),
}
