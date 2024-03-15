use std::fmt::{Display, Formatter};
use derive_is_enum_variant::is_enum_variant;
use serde::{Deserialize, Serialize};
use enum_compatability_macro::document_compatibility;
use uniplate_derive::Uniplate;
use uniplate::uniplate::Uniplate;
use crate::ast::constants::Constant;
use crate::ast::symbol_table::{Name, SymbolTable};
use crate::metadata::Metadata;

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
            Expression::Reference(_, name) => vars.get(name).and_then(|v| v.domain.min_max_i32()),
            Expression::Constant(_, Constant::Int(i)) => Some((*i, *i)),
            Expression::Sum(_, exprs) => {
                if exprs.is_empty() {
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
                if exprs.is_empty() {
                    return None;
                }
                let bounds = exprs
                    .iter()
                    .map(|e| e.bounds(vars))
                    .collect::<Option<Vec<(i32, i32)>>>()?;
                Some((
                    bounds.iter().map(|(min, _)| *min).min()?,
                    bounds.iter().map(|(_, max)| *max).min()?,
                ))
            }
            _ => todo!(),
        }
    }
}

fn display_expressions(expressions: &[Expression]) -> String {
    if expressions.len() <= 3 {
        format!(
            "[{}]",
            expressions
                .iter()
                .map(|e| e.to_string())
                .collect::<Vec<String>>()
                .join(", ")
        )
    } else {
        format!(
            "[{}..{}]",
            expressions[0],
            expressions[expressions.len() - 1]
        )
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
