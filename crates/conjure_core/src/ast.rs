use serde::{Deserialize, Serialize};
use serde_with::serde_as;
use std::collections::HashMap;
use std::fmt::{Debug, Display, Formatter};

#[serde_as]
#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
pub struct Model {
    #[serde_as(as = "Vec<(_, _)>")]
    pub variables: HashMap<Name, DecisionVariable>,
    pub constraints: Vec<Expression>,
}

impl Model {
    pub fn new() -> Model {
        Model {
            variables: HashMap::new(),
            constraints: Vec::new(),
        }
    }
    // Function to update a DecisionVariable based on its Name
    pub fn update_domain(&mut self, name: &Name, new_domain: Domain) {
        if let Some(decision_var) = self.variables.get_mut(name) {
            decision_var.domain = new_domain;
        }
    }
    // Function to add a new DecisionVariable to the Model
    pub fn add_variable(&mut self, name: Name, decision_var: DecisionVariable) {
        self.variables.insert(name, decision_var);
    }

    pub fn add_constraint(&mut self, expression: Expression) {
        // ToDo (gs248) - there is no checking whatsoever
        // We need to properly validate the expression but this is just for testing
        self.constraints.push(expression);
    }
}

impl Default for Model {
    fn default() -> Self {
        Self::new()
    }
}

#[derive(Clone, Debug, Eq, PartialEq, Ord, PartialOrd, Hash, Serialize, Deserialize)]
pub enum Name {
    UserName(String),
    MachineName(i32),
}

impl Display for Name {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            Name::UserName(s) => write!(f, "UserName({})", s),
            Name::MachineName(i) => write!(f, "MachineName({})", i),
        }
    }
}

#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
pub struct DecisionVariable {
    pub domain: Domain,
}

impl Display for DecisionVariable {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match &self.domain {
            Domain::BoolDomain => write!(f, "bool"),
            Domain::IntDomain(ranges) => {
                let mut first = true;
                for r in ranges {
                    if first {
                        first = false;
                    } else {
                        write!(f, " or ")?;
                    }
                    match r {
                        Range::Single(i) => write!(f, "{}", i)?,
                        Range::Bounded(i, j) => write!(f, "{}..{}", i, j)?,
                    }
                }
                Ok(())
            }
        }
    }
}

#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
pub enum Domain {
    BoolDomain,
    IntDomain(Vec<Range<i32>>),
}

#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
pub enum Range<A> {
    Single(A),
    Bounded(A, A),
}

#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
#[non_exhaustive]
pub enum Expression {
    ConstantInt(i32),
    ConstantBool(bool),

    Reference(Name),

    Sum(Vec<Expression>),

    Not(Box<Expression>),
    Or(Vec<Expression>),
    And(Vec<Expression>),

    Eq(Box<Expression>, Box<Expression>),
    Neq(Box<Expression>, Box<Expression>),
    Geq(Box<Expression>, Box<Expression>),
    Leq(Box<Expression>, Box<Expression>),
    Gt(Box<Expression>, Box<Expression>),
    Lt(Box<Expression>, Box<Expression>),

    // Flattened Constraints
    SumGeq(Vec<Expression>, Box<Expression>),
    SumLeq(Vec<Expression>, Box<Expression>),
    Ineq(Box<Expression>, Box<Expression>, Box<Expression>),
}

fn display_expressions(expressions: &Vec<Expression>) -> String {
    if expressions.len() <= 3 {
        format!(
            "Sum({})",
            expressions
                .iter()
                .map(|e| e.to_string())
                .collect::<Vec<String>>()
                .join(", ")
        )
    } else {
        format!(
            "Sum({}..{})",
            expressions[0],
            expressions[expressions.len() - 1]
        )
    }
}

impl Display for Expression {
    fn fmt(&self, f: &mut Formatter<'_>) -> std::fmt::Result {
        match &self {
            Expression::ConstantInt(i) => write!(f, "ConstantInt({})", i),
            Expression::ConstantBool(b) => write!(f, "ConstantBool({})", b),
            Expression::Reference(name) => write!(f, "Reference({})", name),
            Expression::Sum(expressions) => write!(f, "Sum({})", display_expressions(expressions)),
            Expression::Not(expr_box) => write!(f, "Not({})", expr_box.clone()),
            Expression::Or(expressions) => write!(f, "Not({})", display_expressions(expressions)),
            Expression::And(expressions) => write!(f, "And({})", display_expressions(expressions)),
            Expression::Eq(box1, box2) => write!(f, "Eq({}, {})", box1.clone(), box2.clone()),
            Expression::Neq(box1, box2) => write!(f, "Neq({}, {})", box1.clone(), box2.clone()),
            Expression::Geq(box1, box2) => write!(f, "Geq({}, {})", box1.clone(), box2.clone()),
            Expression::Leq(box1, box2) => write!(f, "Leq({}, {})", box1.clone(), box2.clone()),
            Expression::Gt(box1, box2) => write!(f, "Gt({}, {})", box1.clone(), box2.clone()),
            Expression::Lt(box1, box2) => write!(f, "Lt({}, {})", box1.clone(), box2.clone()),
            Expression::SumGeq(box1, box2) => {
                write!(f, "SumGeq({}, {})", display_expressions(box1), box2.clone())
            }
            Expression::SumLeq(box1, box2) => {
                write!(f, "SumLeq({}, {})", display_expressions(box1), box2.clone())
            }
            Expression::Ineq(box1, box2, box3) => write!(
                f,
                "Ineq({}, {}, {})",
                box1.clone(),
                box2.clone(),
                box3.clone()
            ),
            _ => write!(f, "Expression::Unknown"),
        }
    }
}
