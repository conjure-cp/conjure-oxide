use derive_is_enum_variant::is_enum_variant;
use enum_compatability_macro::document_compatibility;
use serde::{Deserialize, Serialize};
use serde_with::serde_as;
use std::collections::HashMap;
use std::fmt::{Debug, Display, Formatter};
use std::hash::Hash;

use crate::metadata::Metadata;

pub type SymbolTable = HashMap<Name, DecisionVariable>;

#[serde_as]
#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
pub struct Model {
    #[serde_as(as = "Vec<(_, _)>")]
    pub variables: SymbolTable,
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

#[document_compatibility]
#[derive(Clone, Debug, PartialEq, is_enum_variant, Serialize, Deserialize)]
#[non_exhaustive]
pub enum Expression {
    /**
     * Represents an empty expression
     * NB: we only expect this at the top level of a model (if there is no constraints)
     */
    Nothing,

    #[compatible(Minion, JsonInput)]
    Constant(Metadata, Constant),

    #[compatible(Minion, JsonInput, SAT)]
    Reference(Metadata, Name),

    #[compatible(Minion, JsonInput)]
    Sum(Metadata, Vec<Expression>),

    // /// Division after preventing division by zero, usually with a top-level constraint
    // #[compatible(Minion)]
    // SafeDiv(Metadata, Box<Expression>, Box<Expression>),
    // /// Division with a possibly undefined value (division by 0)
    // #[compatible(Minion, JsonInput)]
    // Div(Metadata, Box<Expression>, Box<Expression>),
    #[compatible(JsonInput)]
    Min(Metadata, Vec<Expression>),

    #[compatible(JsonInput, SAT)]
    Not(Metadata, Box<Expression>),

    #[compatible(JsonInput, SAT)]
    Or(Metadata, Vec<Expression>),

    #[compatible(JsonInput, SAT)]
    And(Metadata, Vec<Expression>),

    #[compatible(JsonInput)]
    Eq(Metadata, Box<Expression>, Box<Expression>),

    #[compatible(JsonInput)]
    Neq(Metadata, Box<Expression>, Box<Expression>),

    #[compatible(JsonInput)]
    Geq(Metadata, Box<Expression>, Box<Expression>),

    #[compatible(JsonInput)]
    Leq(Metadata, Box<Expression>, Box<Expression>),

    #[compatible(JsonInput)]
    Gt(Metadata, Box<Expression>, Box<Expression>),

    #[compatible(JsonInput)]
    Lt(Metadata, Box<Expression>, Box<Expression>),

    /* Flattened SumEq.
     *
     * Note: this is an intermediary step that's used in the process of converting from conjure model to minion.
     * This is NOT a valid expression in either Essence or minion.
     *
     * ToDo: This is a stop gap solution. Eventually it may be better to have multiple constraints instead? (gs248)
     */
    SumEq(Metadata, Vec<Expression>, Box<Expression>),

    // Flattened Constraints
    #[compatible(Minion)]
    SumGeq(Metadata, Vec<Expression>, Box<Expression>),

    #[compatible(Minion)]
    SumLeq(Metadata, Vec<Expression>, Box<Expression>),

    #[compatible(Minion)]
    Ineq(Metadata, Box<Expression>, Box<Expression>, Box<Expression>),

    #[compatible(Minion)]
    DivEq(Metadata, Box<Expression>, Box<Expression>, Box<Expression>),

    #[compatible(Minion)]
    AllDiff(Metadata, Vec<Expression>),
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
            lhs: &'a [Expression],
            rhs: &'a Box<Expression>,
        ) -> Vec<&'a Expression> {
            let mut sub_exprs = lhs.iter().collect::<Vec<_>>();
            sub_exprs.push(rhs.as_ref());
            sub_exprs
        }

        match self {
            Expression::Constant(_, _) => None,
            Expression::Reference(_, _) => None,
            Expression::Nothing => None,
            Expression::Sum(_, exprs) => Some(exprs.iter().collect()),
            // Expression::SafeDiv(_, lhs, rhs) => Some(vec![lhs.as_ref(), rhs.as_ref()]),
            // Expression::Div(_, lhs, rhs) => Some(vec![lhs.as_ref(), rhs.as_ref()]),
            Expression::Min(_, exprs) => Some(exprs.iter().collect()),
            Expression::Not(_, expr_box) => Some(vec![expr_box.as_ref()]),
            Expression::Or(_, exprs) => Some(exprs.iter().collect()),
            Expression::And(_, exprs) => Some(exprs.iter().collect()),
            Expression::Eq(_, lhs, rhs) => Some(vec![lhs.as_ref(), rhs.as_ref()]),
            Expression::Neq(_, lhs, rhs) => Some(vec![lhs.as_ref(), rhs.as_ref()]),
            Expression::Geq(_, lhs, rhs) => Some(vec![lhs.as_ref(), rhs.as_ref()]),
            Expression::Leq(_, lhs, rhs) => Some(vec![lhs.as_ref(), rhs.as_ref()]),
            Expression::Gt(_, lhs, rhs) => Some(vec![lhs.as_ref(), rhs.as_ref()]),
            Expression::Lt(_, lhs, rhs) => Some(vec![lhs.as_ref(), rhs.as_ref()]),
            Expression::SumGeq(_, lhs, rhs) => Some(unwrap_flat_expression(lhs, rhs)),
            Expression::SumLeq(_, lhs, rhs) => Some(unwrap_flat_expression(lhs, rhs)),
            Expression::SumEq(_, lhs, rhs) => Some(unwrap_flat_expression(lhs, rhs)),
            Expression::Ineq(_, lhs, rhs, _) => Some(vec![lhs.as_ref(), rhs.as_ref()]),
            Expression::DivEq(_, lhs, rhs, _) => Some(vec![lhs.as_ref(), rhs.as_ref()]),
            Expression::AllDiff(_, exprs) => Some(exprs.iter().collect()),
        }
    }

    /// Returns a clone of the same expression type with the given sub-expressions.
    pub fn with_sub_expressions(&self, sub: Vec<&Expression>) -> Expression {
        match self {
            Expression::Constant(metadata, c) => Expression::Constant(metadata.clone(), c.clone()),
            Expression::Reference(metadata, name) => {
                Expression::Reference(metadata.clone(), name.clone())
            }
            Expression::Nothing => Expression::Nothing,
            Expression::Sum(metadata, _) => {
                Expression::Sum(metadata.clone(), sub.iter().cloned().cloned().collect())
            }
            // Expression::Div(metadata, _, _) => Expression::Div(
            //     metadata.clone(),
            //     Box::new(sub[0].clone()),
            //     Box::new(sub[1].clone()),
            // ),
            // Expression::SafeDiv(metadata, _, _) => Expression::SafeDiv(
            //     metadata.clone(),
            //     Box::new(sub[0].clone()),
            //     Box::new(sub[1].clone()),
            // ),
            Expression::Min(metadata, _) => {
                Expression::Min(metadata.clone(), sub.iter().cloned().cloned().collect())
            }
            Expression::Not(metadata, _) => {
                Expression::Not(metadata.clone(), Box::new(sub[0].clone()))
            }
            Expression::Or(metadata, _) => {
                Expression::Or(metadata.clone(), sub.iter().cloned().cloned().collect())
            }
            Expression::And(metadata, _) => {
                Expression::And(metadata.clone(), sub.iter().cloned().cloned().collect())
            }
            Expression::Eq(metadata, _, _) => Expression::Eq(
                metadata.clone(),
                Box::new(sub[0].clone()),
                Box::new(sub[1].clone()),
            ),
            Expression::Neq(metadata, _, _) => Expression::Neq(
                metadata.clone(),
                Box::new(sub[0].clone()),
                Box::new(sub[1].clone()),
            ),
            Expression::Geq(metadata, _, _) => Expression::Geq(
                metadata.clone(),
                Box::new(sub[0].clone()),
                Box::new(sub[1].clone()),
            ),
            Expression::Leq(metadata, _, _) => Expression::Leq(
                metadata.clone(),
                Box::new(sub[0].clone()),
                Box::new(sub[1].clone()),
            ),
            Expression::Gt(metadata, _, _) => Expression::Gt(
                metadata.clone(),
                Box::new(sub[0].clone()),
                Box::new(sub[1].clone()),
            ),
            Expression::Lt(metadata, _, _) => Expression::Lt(
                metadata.clone(),
                Box::new(sub[0].clone()),
                Box::new(sub[1].clone()),
            ),
            Expression::SumGeq(metadata, _, _) => Expression::SumGeq(
                metadata.clone(),
                sub.iter().cloned().cloned().collect(),
                Box::new(sub[2].clone()), // ToDo (gs248) - Why are we using sub[2] here?
            ),
            Expression::SumLeq(metadata, _, _) => Expression::SumLeq(
                metadata.clone(),
                sub.iter().cloned().cloned().collect(),
                Box::new(sub[2].clone()),
            ),
            Expression::SumEq(metadata, _, _) => Expression::SumEq(
                metadata.clone(),
                sub.iter().cloned().cloned().collect(),
                Box::new(sub[2].clone()),
            ),
            Expression::Ineq(metadata, _, _, _) => Expression::Ineq(
                metadata.clone(),
                Box::new(sub[0].clone()),
                Box::new(sub[1].clone()),
                Box::new(sub[2].clone()),
            ),
            Expression::DivEq(metadata, _, _, _) => Expression::DivEq(
                metadata.clone(),
                Box::new(sub[0].clone()),
                Box::new(sub[1].clone()),
                Box::new(sub[2].clone()),
            ),
            Expression::AllDiff(metadata, _) => {
                Expression::AllDiff(metadata.clone(), sub.iter().cloned().cloned().collect())
            }
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
            Expression::Constant(metadata, c) => write!(f, "Constant({}, {})", metadata, c),
            Expression::Reference(metadata, name) => write!(f, "Reference({}, {})", metadata, name),
            Expression::Nothing => write!(f, "Nothing"),
            Expression::Sum(metadata, expressions) => {
                write!(f, "Sum({}, {})", metadata, display_expressions(expressions))
            }
            Expression::Not(metadata, expr_box) => {
                write!(f, "Not({}, {})", metadata, expr_box.clone())
            }
            Expression::Or(metadata, expressions) => {
                write!(f, "Not({}, {})", metadata, display_expressions(expressions))
            }
            Expression::And(metadata, expressions) => {
                write!(f, "And({}, {})", metadata, display_expressions(expressions))
            }
            Expression::Eq(metadata, box1, box2) => {
                write!(f, "Eq({}, {}, {})", metadata, box1.clone(), box2.clone())
            }
            Expression::Neq(metadata, box1, box2) => {
                write!(f, "Neq({}, {}, {})", metadata, box1.clone(), box2.clone())
            }
            Expression::Geq(metadata, box1, box2) => {
                write!(f, "Geq({}, {}, {})", metadata, box1.clone(), box2.clone())
            }
            Expression::Leq(metadata, box1, box2) => {
                write!(f, "Leq({}, {}, {})", metadata, box1.clone(), box2.clone())
            }
            Expression::Gt(metadata, box1, box2) => {
                write!(f, "Gt({}, {}, {})", metadata, box1.clone(), box2.clone())
            }
            Expression::Lt(metadata, box1, box2) => {
                write!(f, "Lt({}, {}, {})", metadata, box1.clone(), box2.clone())
            }
            Expression::SumGeq(metadata, box1, box2) => {
                write!(
                    f,
                    "SumGeq({}, {}. {})",
                    metadata,
                    display_expressions(box1),
                    box2.clone()
                )
            }
            Expression::SumLeq(metadata, box1, box2) => {
                write!(
                    f,
                    "SumLeq({}, {}, {})",
                    metadata,
                    display_expressions(box1),
                    box2.clone()
                )
            }
            Expression::Ineq(metadata, box1, box2, box3) => write!(
                f,
                "Ineq({}, {}, {}, {})",
                metadata,
                box1.clone(),
                box2.clone(),
                box3.clone()
            ),
            #[allow(unreachable_patterns)]
            _ => write!(f, "Expression::Unknown"),
        }
    }
}
