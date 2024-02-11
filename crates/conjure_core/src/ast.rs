use doc_solver_support::doc_solver_support;
use serde::{Deserialize, Serialize};
use serde_with::serde_as;
use std::collections::HashMap;
use std::fmt::{Debug, Display, Formatter};

#[serde_as]
#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
pub struct Model {
    #[serde_as(as = "Vec<(_, _)>")]
    pub variables: HashMap<Name, DecisionVariable>,
    pub constraints: Expression,
}

impl Model {
    pub fn new() -> Model {
        Model {
            variables: HashMap::new(),
            constraints: Expression::Nothing,
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

    pub fn get_constraints_vec(&self) -> Vec<Expression> {
        match &self.constraints {
            Expression::And(constraints) => constraints.clone(),
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
            self.constraints = Expression::And(constraints);
        }
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
pub enum Constant {
    Int(i32),
    Bool(bool),
}

impl TryFrom<Constant> for i32 {
    type Error = &'static str;

    fn try_from(value: Constant) -> Result<Self, Self::Error> {
        match value {
            Constant::Int(i) => Ok(i),
            _ => Err("Cannot convert non-i32 Constant to i32"),
        }
    }
}
impl TryFrom<Constant> for bool {
    type Error = &'static str;

    fn try_from(value: Constant) -> Result<Self, Self::Error> {
        match value {
            Constant::Bool(b) => Ok(b),
            _ => Err("Cannot convert non-bool Constant to bool"),
        }
    }
}

#[doc_solver_support]
#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
#[non_exhaustive]
pub enum Expression {
    /**
     * Represents an empty expression
     * NB: we only expect this at the top level of a model (if there is no constraints)
     */
    Nothing,

    #[solver(Minion, SAT)]
    Constant(Constant),

    #[solver(Minion)]
    Reference(Name),

    Sum(Vec<Expression>),

    #[solver(SAT)]
    Not(Box<Expression>),
    #[solver(SAT)]
    Or(Vec<Expression>),
    #[solver(SAT)]
    And(Vec<Expression>),

    Eq(Box<Expression>, Box<Expression>),
    Neq(Box<Expression>, Box<Expression>),
    Geq(Box<Expression>, Box<Expression>),
    Leq(Box<Expression>, Box<Expression>),
    Gt(Box<Expression>, Box<Expression>),
    Lt(Box<Expression>, Box<Expression>),

    /* Flattened SumEq.
     *
     * Note: this is an intermediary step that's used in the process of converting from conjure model to minion.
     * This is NOT a valid expression in either Essence or minion.
     *
     * ToDo: This is a stop gap solution. Eventually it may be better to have multiple constraints instead? (gs248)
     */
    SumEq(Vec<Expression>, Box<Expression>),

    // Flattened Constraints
    #[solver(Minion)]
    SumGeq(Vec<Expression>, Box<Expression>),
    #[solver(Minion)]
    SumLeq(Vec<Expression>, Box<Expression>),
    #[solver(Minion)]
    Ineq(Box<Expression>, Box<Expression>, Box<Expression>),
}

impl Expression {
    /**
     * Returns a vector of references to the sub-expressions of the expression.
     * If the expression is a primitive (variable, constant, etc.), returns None.
     *
     * Note: If the expression is NOT MEANT TO have sub-expressions, this function will return None.
     * Otherwise, it will return Some(Vec), where the Vec can be empty.
     */
    pub fn sub_expressions(&self) -> Option<Vec<&Expression>> {
        fn unwrap_flat_expression<'a>(
            lhs: &'a Vec<Expression>,
            rhs: &'a Box<Expression>,
        ) -> Vec<&'a Expression> {
            let mut sub_exprs = lhs.iter().collect::<Vec<_>>();
            sub_exprs.push(rhs.as_ref());
            sub_exprs
        }

        match self {
            Expression::Constant(_) => None,
            Expression::Reference(_) => None,
            Expression::Nothing => None,
            Expression::Sum(exprs) => Some(exprs.iter().collect()),
            Expression::Not(expr_box) => Some(vec![expr_box.as_ref()]),
            Expression::Or(exprs) => Some(exprs.iter().collect()),
            Expression::And(exprs) => Some(exprs.iter().collect()),
            Expression::Eq(lhs, rhs) => Some(vec![lhs.as_ref(), rhs.as_ref()]),
            Expression::Neq(lhs, rhs) => Some(vec![lhs.as_ref(), rhs.as_ref()]),
            Expression::Geq(lhs, rhs) => Some(vec![lhs.as_ref(), rhs.as_ref()]),
            Expression::Leq(lhs, rhs) => Some(vec![lhs.as_ref(), rhs.as_ref()]),
            Expression::Gt(lhs, rhs) => Some(vec![lhs.as_ref(), rhs.as_ref()]),
            Expression::Lt(lhs, rhs) => Some(vec![lhs.as_ref(), rhs.as_ref()]),
            Expression::SumGeq(lhs, rhs) => Some(unwrap_flat_expression(lhs, rhs)),
            Expression::SumLeq(lhs, rhs) => Some(unwrap_flat_expression(lhs, rhs)),
            Expression::SumEq(lhs, rhs) => Some(unwrap_flat_expression(lhs, rhs)),
            Expression::Ineq(lhs, rhs, _) => Some(vec![lhs.as_ref(), rhs.as_ref()]),
        }
    }

    /// Returns a clone of the same expression type with the given sub-expressions.
    pub fn with_sub_expressions(&self, sub: Vec<&Expression>) -> Expression {
        match self {
            Expression::Constant(c) => Expression::Constant(c.clone()),
            Expression::Reference(name) => Expression::Reference(name.clone()),
            Expression::Nothing => Expression::Nothing,
            Expression::Sum(_) => Expression::Sum(sub.iter().cloned().cloned().collect()),
            Expression::Not(_) => Expression::Not(Box::new(sub[0].clone())),
            Expression::Or(_) => Expression::Or(sub.iter().cloned().cloned().collect()),
            Expression::And(_) => Expression::And(sub.iter().cloned().cloned().collect()),
            Expression::Eq(_, _) => {
                Expression::Eq(Box::new(sub[0].clone()), Box::new(sub[1].clone()))
            }
            Expression::Neq(_, _) => {
                Expression::Neq(Box::new(sub[0].clone()), Box::new(sub[1].clone()))
            }
            Expression::Geq(_, _) => {
                Expression::Geq(Box::new(sub[0].clone()), Box::new(sub[1].clone()))
            }
            Expression::Leq(_, _) => {
                Expression::Leq(Box::new(sub[0].clone()), Box::new(sub[1].clone()))
            }
            Expression::Gt(_, _) => {
                Expression::Gt(Box::new(sub[0].clone()), Box::new(sub[1].clone()))
            }
            Expression::Lt(_, _) => {
                Expression::Lt(Box::new(sub[0].clone()), Box::new(sub[1].clone()))
            }
            Expression::SumGeq(_, _) => Expression::SumGeq(
                sub.iter().cloned().cloned().collect(),
                Box::new(sub[2].clone()), // ToDo (gs248) - Why are we using sub[2] here?
            ),
            Expression::SumLeq(_, _) => Expression::SumLeq(
                sub.iter().cloned().cloned().collect(),
                Box::new(sub[2].clone()),
            ),
            Expression::SumEq(_, _) => Expression::SumEq(
                sub.iter().cloned().cloned().collect(),
                Box::new(sub[2].clone()),
            ),
            Expression::Ineq(_, _, _) => Expression::Ineq(
                Box::new(sub[0].clone()),
                Box::new(sub[1].clone()),
                Box::new(sub[2].clone()),
            ),
        }
    }

    pub fn is_constant(&self) -> bool {
        match self {
            Expression::Constant(_) => true,
            _ => false,
        }
    }
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

impl Display for Constant {
    fn fmt(&self, f: &mut Formatter<'_>) -> std::fmt::Result {
        match &self {
            Constant::Int(i) => write!(f, "Int({})", i),
            Constant::Bool(b) => write!(f, "Bool({})", b),
        }
    }
}

impl Display for Expression {
    fn fmt(&self, f: &mut Formatter<'_>) -> std::fmt::Result {
        match &self {
            Expression::Constant(c) => write!(f, "Constant::{}", c),
            Expression::Reference(name) => write!(f, "Reference({})", name),
            Expression::Nothing => write!(f, "Nothing"),
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
            #[allow(unreachable_patterns)]
            _ => write!(f, "Expression::Unknown"),
        }
    }
}
