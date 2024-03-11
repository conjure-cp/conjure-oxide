use derive_is_enum_variant::is_enum_variant;
use enum_compatability_macro::document_compatibility;
use serde::{Deserialize, Serialize};
use serde_with::serde_as;
use std::cell::RefCell;
use std::collections::HashMap;
use std::fmt::{Debug, Display, Formatter};
use std::hash::Hash;
use uniplate::uniplate::Uniplate;
use uniplate_derive::Uniplate;

use crate::metadata::Metadata;

pub type SymbolTable = HashMap<Name, DecisionVariable>;

#[serde_as]
#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub struct Model {
    #[serde_as(as = "Vec<(_, _)>")]
    pub variables: SymbolTable,
    pub constraints: Expression,
    next_var: RefCell<i32>,
}

impl Model {
    pub fn new(variables: SymbolTable, constraints: Expression) -> Model {
        Model {
            variables: variables,
            constraints: constraints,
            next_var: RefCell::new(0),
        }
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
        let num = self.next_var.borrow().clone();
        *(self.next_var.borrow_mut()) += 1;
        Name::MachineName(num) // incremented when inserted
    }
}

impl Default for Model {
    fn default() -> Self {
        Self::new(SymbolTable::new(), Expression::Nothing)
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

#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub struct DecisionVariable {
    pub domain: Domain,
}

impl DecisionVariable {
    pub fn new(domain: Domain) -> DecisionVariable {
        DecisionVariable { domain }
    }
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

#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub enum Domain {
    BoolDomain,
    IntDomain(Vec<Range<i32>>),
}

impl Domain {
    /// Returns the minimum i32 value a variable of the domain can take, if it is an i32 domain.
    pub fn min_i32(&self) -> Option<i32> {
        match self {
            Domain::BoolDomain => Some(0),
            Domain::IntDomain(ranges) => {
                if ranges.is_empty() {
                    return None;
                }
                let mut min = i32::MAX;
                for r in ranges {
                    match r {
                        Range::Single(i) => min = min.min(*i),
                        Range::Bounded(i, _) => min = min.min(*i),
                    }
                }
                Some(min)
            }
        }
    }

    /// Returns the maximum i32 value a variable of the domain can take, if it is an i32 domain.
    pub fn max_i32(&self) -> Option<i32> {
        match self {
            Domain::BoolDomain => Some(1),
            Domain::IntDomain(ranges) => {
                if ranges.is_empty() {
                    return None;
                }
                let mut max = i32::MIN;
                for r in ranges {
                    match r {
                        Range::Single(i) => max = max.max(*i),
                        Range::Bounded(_, i) => max = max.max(*i),
                    }
                }
                Some(max)
            }
        }
    }

    /// Returns the minimum and maximum integer values a variable of the domain can take, if it is an integer domain.
    pub fn min_max_i32(&self) -> Option<(i32, i32)> {
        match self {
            Domain::BoolDomain => Some((0, 1)),
            Domain::IntDomain(ranges) => {
                if ranges.is_empty() {
                    return None;
                }
                let mut min = i32::MAX;
                let mut max = i32::MIN;
                for r in ranges {
                    match r {
                        Range::Single(i) => {
                            min = min.min(*i);
                            max = max.max(*i);
                        }
                        Range::Bounded(i, j) => {
                            min = min.min(*i);
                            max = max.max(*j);
                        }
                    }
                }
                Some((min, max))
            }
        }
    }
}

#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub enum Range<A> {
    Single(A),
    Bounded(A, A),
}

#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
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
#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize, is_enum_variant, Uniplate)]
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

    // #[compatible(Minion)]
    // DivEq(Metadata, Box<Expression>, Box<Expression>, Box<Expression>),
    #[compatible(Minion)]
    AllDiff(Metadata, Vec<Expression>),
}

impl Expression {
    pub fn bounds(&self, vars: &SymbolTable) -> Option<(i32, i32)> {
        match self {
            Expression::Reference(_, name) => vars.get(name).and_then(|v| {
                let b = v.domain.min_max_i32();
                b
            }),
            Expression::Constant(_, Constant::Int(i)) => Some((*i, *i)),
            Expression::Sum(_, exprs) => {
                if exprs.len() == 0 {
                    return None;
                }
                let (mut min, mut max) = (0, 0);
                for e in exprs {
                    if let Some((e_min, e_max)) = e.bounds(vars) {
                        min += e_min;
                        max += e_max;
                    } else {
                        return None;
                    }
                }
                Some((min, max))
            }
            Expression::Min(_, exprs) => {
                if exprs.len() == 0 {
                    return None;
                }
                let bounds = exprs
                    .iter()
                    .map(|e| e.bounds(vars))
                    .collect::<Option<Vec<(i32, i32)>>>()?;
                Some((
                    bounds.iter().map(|(min, _)| *min).min().unwrap(),
                    bounds.iter().map(|(_, max)| *max).min().unwrap(),
                ))
            }
            _ => todo!(),
        }
    }
}

fn display_expressions(expressions: &[Expression]) -> String {
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
