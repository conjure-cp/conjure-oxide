use std::fmt::{Display, Formatter};

use serde::{Deserialize, Serialize};

use enum_compatability_macro::document_compatibility;
use uniplate::derive::Uniplate;
use uniplate::{Biplate, Uniplate};

use crate::ast::constants::Constant;
use crate::ast::symbol_table::{Name, SymbolTable};
use crate::ast::ReturnType;
use crate::metadata::Metadata;

use super::{Domain, Range};

/// Represents different types of expressions used to define rules and constraints in the model.
///
/// The `Expression` enum includes operations, constants, and variable references
/// used to build rules and conditions for the model.
#[document_compatibility]
#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize, Uniplate)]
#[uniplate(walk_into=[])]
#[biplate(to=Constant)]
pub enum Expression {
    /**
     * Represents an empty expression
     * NB: we only expect this at the top level of a model (if there is no constraints)
     */
    Nothing,

    /// An expression representing "A is valid as long as B is true"
    /// Turns into a conjunction when it reaches a boolean context
    Bubble(Metadata, Box<Expression>, Box<Expression>),

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

    /// Division after preventing division by zero, usually with a bubble
    SafeDiv(Metadata, Box<Expression>, Box<Expression>),

    /// Division with a possibly undefined value (division by 0)
    #[compatible(JsonInput)]
    UnsafeDiv(Metadata, Box<Expression>, Box<Expression>),

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
    DivEq(Metadata, Box<Expression>, Box<Expression>, Box<Expression>),

    #[compatible(Minion)]
    Ineq(Metadata, Box<Expression>, Box<Expression>, Box<Expression>),

    #[compatible(Minion)]
    AllDiff(Metadata, Vec<Expression>),
}

fn expr_vec_to_domain_i32(
    exprs: &Vec<Expression>,
    op: fn(i32, i32) -> Option<i32>,
    vars: &SymbolTable,
) -> Option<Domain> {
    let domains: Vec<Option<_>> = exprs.iter().map(|e| e.domain_of(vars)).collect();
    domains
        .into_iter()
        .reduce(|a, b| a.and_then(|x| b.and_then(|y| x.apply_i32(op, &y))))
        .flatten()
}

fn range_vec_bounds_i32(ranges: &Vec<Range<i32>>) -> (i32, i32) {
    let mut min = i32::MAX;
    let mut max = i32::MIN;
    for r in ranges {
        match r {
            Range::Single(i) => {
                if *i < min {
                    min = *i;
                }
                if *i > max {
                    max = *i;
                }
            }
            Range::Bounded(i, j) => {
                if *i < min {
                    min = *i;
                }
                if *j > max {
                    max = *j;
                }
            }
        }
    }
    (min, max)
}

impl Expression {
    /// Returns the possible values of the expression, recursing to leaf expressions
    pub fn domain_of(&self, vars: &SymbolTable) -> Option<Domain> {
        let ret = match self {
            Expression::Reference(_, name) => Some(vars.get(name)?.domain.clone()),
            Expression::Constant(_, Constant::Int(n)) => {
                Some(Domain::IntDomain(vec![Range::Single(*n)]))
            }
            Expression::Constant(_, Constant::Bool(_)) => Some(Domain::BoolDomain),
            Expression::Sum(_, exprs) => expr_vec_to_domain_i32(exprs, |x, y| Some(x + y), vars),
            Expression::Min(_, exprs) => {
                expr_vec_to_domain_i32(exprs, |x, y| Some(if x < y { x } else { y }), vars)
            }
            Expression::UnsafeDiv(_, a, b) | Expression::SafeDiv(_, a, b) => {
                a.domain_of(vars)?.apply_i32(
                    |x, y| if y != 0 { Some(x / y) } else { None },
                    &b.domain_of(vars)?,
                )
            }
            _ => todo!("Calculate domain of {:?}", self),
            // TODO: (flm8) Add support for calculating the domains of more expression types
        };
        match ret {
            // TODO: (flm8) the Minion bindings currently only support single ranges for domains, so we use the min/max bounds
            // Once they support a full domain as we define it, we can remove this conversion
            Some(Domain::IntDomain(ranges)) if ranges.len() > 1 => {
                let (min, max) = range_vec_bounds_i32(&ranges);
                Some(Domain::IntDomain(vec![Range::Bounded(min, max)]))
            }
            _ => ret,
        }
    }

    pub fn can_be_undefined(&self) -> bool {
        // TODO: there will be more false cases but we are being conservative
        match self {
            Expression::Reference(_, _) => false,
            Expression::Constant(_, Constant::Bool(_)) => false,
            Expression::Constant(_, Constant::Int(_)) => false,
            _ => true,
        }
    }

    pub fn return_type(&self) -> Option<ReturnType> {
        match self {
            Expression::Constant(_, Constant::Int(_)) => Some(ReturnType::Int),
            Expression::Constant(_, Constant::Bool(_)) => Some(ReturnType::Bool),
            Expression::Reference(_, _) => None,
            Expression::Sum(_, _) => Some(ReturnType::Int),
            Expression::Min(_, _) => Some(ReturnType::Int),
            Expression::Not(_, _) => Some(ReturnType::Bool),
            Expression::Or(_, _) => Some(ReturnType::Bool),
            Expression::And(_, _) => Some(ReturnType::Bool),
            Expression::Eq(_, _, _) => Some(ReturnType::Bool),
            Expression::Neq(_, _, _) => Some(ReturnType::Bool),
            Expression::Geq(_, _, _) => Some(ReturnType::Bool),
            Expression::Leq(_, _, _) => Some(ReturnType::Bool),
            Expression::Gt(_, _, _) => Some(ReturnType::Bool),
            Expression::Lt(_, _, _) => Some(ReturnType::Bool),
            Expression::SafeDiv(_, _, _) => Some(ReturnType::Int),
            Expression::UnsafeDiv(_, _, _) => Some(ReturnType::Int),
            Expression::SumEq(_, _, _) => Some(ReturnType::Bool),
            Expression::SumGeq(_, _, _) => Some(ReturnType::Bool),
            Expression::SumLeq(_, _, _) => Some(ReturnType::Bool),
            Expression::DivEq(_, _, _, _) => Some(ReturnType::Bool),
            Expression::Ineq(_, _, _, _) => Some(ReturnType::Bool),
            Expression::AllDiff(_, _) => Some(ReturnType::Bool),
            Expression::Bubble(_, _, _) => None, // TODO: (flm8) should this be a bool?
            Expression::Nothing => None,
        }
    }

    pub fn is_clean(&self) -> bool {
        match self {
            Expression::Nothing => true,
            Expression::Constant(metadata, _) => metadata.clean,
            Expression::Reference(metadata, _) => metadata.clean,
            Expression::Sum(metadata, exprs) => metadata.clean,
            Expression::Min(metadata, exprs) => metadata.clean,
            Expression::Not(metadata, expr) => metadata.clean,
            Expression::Or(metadata, exprs) => metadata.clean,
            Expression::And(metadata, exprs) => metadata.clean,
            Expression::Eq(metadata, box1, box2) => metadata.clean,
            Expression::Neq(metadata, box1, box2) => metadata.clean,
            Expression::Geq(metadata, box1, box2) => metadata.clean,
            Expression::Leq(metadata, box1, box2) => metadata.clean,
            Expression::Gt(metadata, box1, box2) => metadata.clean,
            Expression::Lt(metadata, box1, box2) => metadata.clean,
            Expression::SumGeq(metadata, box1, box2) => metadata.clean,
            Expression::SumLeq(metadata, box1, box2) => metadata.clean,
            Expression::Ineq(metadata, box1, box2, box3) => metadata.clean,
            Expression::AllDiff(metadata, exprs) => metadata.clean,
            Expression::SumEq(metadata, exprs, expr) => metadata.clean,
            _ => false,
        }
    }

    pub fn set_clean(&mut self, bool_value: bool) {
        match self {
            Expression::Nothing => {}
            Expression::Constant(metadata, _) => metadata.clean = bool_value,
            Expression::Reference(metadata, _) => metadata.clean = bool_value,
            Expression::Sum(metadata, _) => {
                metadata.clean = bool_value;
            }
            Expression::Min(metadata, _) => {
                metadata.clean = bool_value;
            }
            Expression::Not(metadata, _) => {
                metadata.clean = bool_value;
            }
            Expression::Or(metadata, _) => {
                metadata.clean = bool_value;
            }
            Expression::And(metadata, _) => {
                metadata.clean = bool_value;
            }
            Expression::Eq(metadata, box1, box2) => {
                metadata.clean = bool_value;
            }
            Expression::Neq(metadata, _box1, _box2) => {
                metadata.clean = bool_value;
            }
            Expression::Geq(metadata, _box1, _box2) => {
                metadata.clean = bool_value;
            }
            Expression::Leq(metadata, _box1, _box2) => {
                metadata.clean = bool_value;
            }
            Expression::Gt(metadata, _box1, _box2) => {
                metadata.clean = bool_value;
            }
            Expression::Lt(metadata, _box1, _box2) => {
                metadata.clean = bool_value;
            }
            Expression::SumGeq(metadata, _box1, _box2) => {
                metadata.clean = bool_value;
            }
            Expression::SumLeq(metadata, _box1, _box2) => {
                metadata.clean = bool_value;
            }
            Expression::Ineq(metadata, _box1, _box2, _box3) => {
                metadata.clean = bool_value;
            }
            Expression::AllDiff(metadata, _exprs) => {
                metadata.clean = bool_value;
            }
            Expression::SumEq(metadata, _exprs, _expr) => {
                metadata.clean = bool_value;
            }
            Expression::Bubble(metadata, box1, box2) => {
                metadata.clean = bool_value;
            }
            Expression::SafeDiv(metadata, box1, box2) => {
                metadata.clean = bool_value;
            }
            Expression::UnsafeDiv(metadata, box1, box2) => {
                metadata.clean = bool_value;
            }
            Expression::DivEq(metadata, box1, box2, box3) => {
                metadata.clean = bool_value;
            }
        }
    }
}

fn display_expressions(expressions: &[Expression]) -> String {
    // if expressions.len() <= 3 {
    format!(
        "[{}]",
        expressions
            .iter()
            .map(|e| e.to_string())
            .collect::<Vec<String>>()
            .join(", ")
    )
    // } else {
    //     format!(
    //         "[{}..{}]",
    //         expressions[0],
    //         expressions[expressions.len() - 1]
    //     )
    // }
}

impl From<i32> for Expression {
    fn from(i: i32) -> Self {
        Expression::Constant(Metadata::new(), Constant::Int(i))
    }
}

impl From<bool> for Expression {
    fn from(b: bool) -> Self {
        Expression::Constant(Metadata::new(), Constant::Bool(b))
    }
}

impl Display for Expression {
    // TODO: (flm8) this will change once we implement a parser (two-way conversion)
    fn fmt(&self, f: &mut Formatter<'_>) -> std::fmt::Result {
        match &self {
            Expression::Constant(_, c) => match c {
                Constant::Bool(b) => write!(f, "{}", b),
                Constant::Int(i) => write!(f, "{}", i),
            },
            Expression::Reference(_, name) => match name {
                Name::MachineName(n) => write!(f, "_{}", n),
                Name::UserName(s) => write!(f, "{}", s),
            },
            Expression::Nothing => write!(f, "Nothing"),
            Expression::Sum(_, expressions) => {
                write!(f, "Sum({})", display_expressions(expressions))
            }
            Expression::Min(_, expressions) => {
                write!(f, "Min({})", display_expressions(expressions))
            }
            Expression::Not(_, expr_box) => {
                write!(f, "Not({})", expr_box.clone())
            }
            Expression::Or(_, expressions) => {
                write!(f, "Or({})", display_expressions(expressions))
            }
            Expression::And(_, expressions) => {
                write!(f, "And({})", display_expressions(expressions))
            }
            Expression::Eq(_, box1, box2) => {
                write!(f, "({} = {})", box1.clone(), box2.clone())
            }
            Expression::Neq(_, box1, box2) => {
                write!(f, "({} != {})", box1.clone(), box2.clone())
            }
            Expression::Geq(_, box1, box2) => {
                write!(f, "({} >= {})", box1.clone(), box2.clone())
            }
            Expression::Leq(_, box1, box2) => {
                write!(f, "({} <= {})", box1.clone(), box2.clone())
            }
            Expression::Gt(_, box1, box2) => {
                write!(f, "({} > {})", box1.clone(), box2.clone())
            }
            Expression::Lt(_, box1, box2) => {
                write!(f, "({} < {})", box1.clone(), box2.clone())
            }
            Expression::SumEq(_, expressions, expr_box) => {
                write!(
                    f,
                    "SumEq({}, {})",
                    display_expressions(expressions),
                    expr_box.clone()
                )
            }
            Expression::SumGeq(_, box1, box2) => {
                write!(f, "SumGeq({}, {})", display_expressions(box1), box2.clone())
            }
            Expression::SumLeq(_, box1, box2) => {
                write!(f, "SumLeq({}, {})", display_expressions(box1), box2.clone())
            }
            Expression::Ineq(_, box1, box2, box3) => write!(
                f,
                "Ineq({}, {}, {})",
                box1.clone(),
                box2.clone(),
                box3.clone()
            ),
            Expression::AllDiff(_, expressions) => {
                write!(f, "AllDiff({})", display_expressions(expressions))
            }
            Expression::Bubble(_, box1, box2) => {
                write!(f, "{{{} @ {}}}", box1.clone(), box2.clone())
            }
            Expression::SafeDiv(_, box1, box2) => {
                write!(f, "SafeDiv({}, {})", box1.clone(), box2.clone())
            }
            Expression::UnsafeDiv(_, box1, box2) => {
                write!(f, "UnsafeDiv({}, {})", box1.clone(), box2.clone())
            }
            Expression::DivEq(_, box1, box2, box3) => {
                write!(
                    f,
                    "DivEq({}, {}, {})",
                    box1.clone(),
                    box2.clone(),
                    box3.clone()
                )
            }
            #[allow(unreachable_patterns)]
            other => todo!("Implement display for {:?}", other),
        }
    }
}

#[cfg(test)]
mod tests {
    use crate::ast::DecisionVariable;

    use super::*;

    #[test]
    fn test_domain_of_constant_sum() {
        let c1 = Expression::Constant(Metadata::new(), Constant::Int(1));
        let c2 = Expression::Constant(Metadata::new(), Constant::Int(2));
        let sum = Expression::Sum(Metadata::new(), vec![c1.clone(), c2.clone()]);
        assert_eq!(
            sum.domain_of(&SymbolTable::new()),
            Some(Domain::IntDomain(vec![Range::Single(3)]))
        );
    }

    #[test]
    fn test_domain_of_constant_invalid_type() {
        let c1 = Expression::Constant(Metadata::new(), Constant::Int(1));
        let c2 = Expression::Constant(Metadata::new(), Constant::Bool(true));
        let sum = Expression::Sum(Metadata::new(), vec![c1.clone(), c2.clone()]);
        assert_eq!(sum.domain_of(&SymbolTable::new()), None);
    }

    #[test]
    fn test_domain_of_empty_sum() {
        let sum = Expression::Sum(Metadata::new(), vec![]);
        assert_eq!(sum.domain_of(&SymbolTable::new()), None);
    }

    #[test]
    fn test_domain_of_reference() {
        let reference = Expression::Reference(Metadata::new(), Name::MachineName(0));
        let mut vars = SymbolTable::new();
        vars.insert(
            Name::MachineName(0),
            DecisionVariable::new(Domain::IntDomain(vec![Range::Single(1)])),
        );
        assert_eq!(
            reference.domain_of(&vars),
            Some(Domain::IntDomain(vec![Range::Single(1)]))
        );
    }

    #[test]
    fn test_domain_of_reference_not_found() {
        let reference = Expression::Reference(Metadata::new(), Name::MachineName(0));
        assert_eq!(reference.domain_of(&SymbolTable::new()), None);
    }

    #[test]
    fn test_domain_of_reference_sum_single() {
        let reference = Expression::Reference(Metadata::new(), Name::MachineName(0));
        let mut vars = SymbolTable::new();
        vars.insert(
            Name::MachineName(0),
            DecisionVariable::new(Domain::IntDomain(vec![Range::Single(1)])),
        );
        let sum = Expression::Sum(Metadata::new(), vec![reference.clone(), reference.clone()]);
        assert_eq!(
            sum.domain_of(&vars),
            Some(Domain::IntDomain(vec![Range::Single(2)]))
        );
    }

    #[test]
    fn test_domain_of_reference_sum_bounded() {
        let reference = Expression::Reference(Metadata::new(), Name::MachineName(0));
        let mut vars = SymbolTable::new();
        vars.insert(
            Name::MachineName(0),
            DecisionVariable::new(Domain::IntDomain(vec![Range::Bounded(1, 2)])),
        );
        let sum = Expression::Sum(Metadata::new(), vec![reference.clone(), reference.clone()]);
        assert_eq!(
            sum.domain_of(&vars),
            Some(Domain::IntDomain(vec![Range::Bounded(2, 4)]))
        );
    }
}
