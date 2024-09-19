// Does Uniplating Conjure-Oxide Expressions Compile?
//
// Using the AST as of 30/04/2024
// -- Niklas Dewally <niklas@dewally.com>

use core::fmt::Display;
use core::fmt::Formatter;
use uniplate::Uniplate;
#[derive(Clone, Debug, PartialEq, Eq, Uniplate)]
#[uniplate()]
#[biplate(to=Constant)]
#[biplate(to=String,walk_into=[Name])]
enum Expression {
    Nothing,
    Bubble(Metadata, Box<Expression>, Box<Expression>),
    Constant(Metadata, Constant),
    Reference(Metadata, Name),
    Sum(Metadata, Vec<Expression>),
    Min(Metadata, Vec<Expression>),
    Not(Metadata, Box<Expression>),
    Or(Metadata, Vec<Expression>),
    And(Metadata, Vec<Expression>),
    Eq(Metadata, Box<Expression>, Box<Expression>),
    Neq(Metadata, Box<Expression>, Box<Expression>),
    Geq(Metadata, Box<Expression>, Box<Expression>),
    Leq(Metadata, Box<Expression>, Box<Expression>),
    Gt(Metadata, Box<Expression>, Box<Expression>),
    Lt(Metadata, Box<Expression>, Box<Expression>),
    SafeDiv(Metadata, Box<Expression>, Box<Expression>),
    UnsafeDiv(Metadata, Box<Expression>, Box<Expression>),
    SumEq(Metadata, Vec<Expression>, Box<Expression>),
    SumGeq(Metadata, Vec<Expression>, Box<Expression>),
    SumLeq(Metadata, Vec<Expression>, Box<Expression>),
    DivEq(Metadata, Box<Expression>, Box<Expression>, Box<Expression>),
    Ineq(Metadata, Box<Expression>, Box<Expression>, Box<Expression>),
    AllDiff(Metadata, Vec<Expression>),
}

#[derive(Clone, Debug, Eq, PartialEq, Ord, PartialOrd, Hash,Uniplate)]
#[uniplate()]
#[biplate(to=String)]
enum Name {
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

#[derive(Clone, Debug, PartialEq, Eq, Default)]
pub struct Metadata {
    pub clean: bool,
}

#[derive(Clone, Debug, PartialEq, Eq,Uniplate)]
#[uniplate()]
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

impl From<i32> for Constant {
    fn from(i: i32) -> Self {
        Constant::Int(i)
    }
}

impl From<bool> for Constant {
    fn from(b: bool) -> Self {
        Constant::Bool(b)
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
pub fn main() {}
