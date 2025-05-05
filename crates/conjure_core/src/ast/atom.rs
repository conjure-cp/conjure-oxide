use std::borrow::Borrow;

use super::{
    literals::AbstractLiteral, records::RecordValue, Expression, Literal, Name, ReturnType,
    Typeable,
};
use serde::{Deserialize, Serialize};
use uniplate::derive::Uniplate;

/// An `Atom` is an indivisible expression, such as a literal or a reference.
#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize, Uniplate)]
#[uniplate()]
#[biplate(to=Literal)]
#[biplate(to=Expression)]
#[biplate(to=AbstractLiteral<Literal>,walk_into=[Literal])]
#[biplate(to=RecordValue<Literal>,walk_into=[Literal])]
#[biplate(to=Name)]
pub enum Atom {
    Literal(Literal),
    Reference(Name),
}

impl Atom {
    /// Shorthand to create a reference by user name.
    pub fn new_uref(name: &str) -> Atom {
        Atom::Reference(Name::UserName(name.to_string()))
    }

    /// Shorthand to create an integer literal.
    pub fn new_ilit(value: i32) -> Atom {
        Atom::Literal(Literal::Int(value))
    }

    /// Shorthand to create a boolean literal.
    pub fn new_blit(value: bool) -> Atom {
        Atom::Literal(Literal::Bool(value))
    }
}

impl Typeable for Atom {
    fn return_type(&self) -> Option<ReturnType> {
        match self {
            Atom::Literal(lit) => lit.return_type(),
            //TODO: access symbol table to get return type of references
            Atom::Reference(_) => None,
        }
    }
}

impl std::fmt::Display for Atom {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            Atom::Literal(x) => x.fmt(f),
            Atom::Reference(x) => x.fmt(f),
        }
    }
}

impl From<Literal> for Atom {
    fn from(value: Literal) -> Self {
        Atom::Literal(value)
    }
}

impl From<Name> for Atom {
    fn from(value: Name) -> Self {
        Atom::Reference(value)
    }
}

impl From<i32> for Atom {
    fn from(value: i32) -> Self {
        Atom::Literal(value.into())
    }
}

impl From<bool> for Atom {
    fn from(value: bool) -> Self {
        Atom::Literal(value.into())
    }
}

impl TryFrom<Expression> for Atom {
    type Error = &'static str;

    fn try_from(value: Expression) -> Result<Self, Self::Error> {
        match value {
            Expression::Atomic(_, atom) => Ok(atom),
            _ => Err("Cannot convert non-atomic expression to Atom"),
        }
    }
}

impl<'a> TryFrom<&'a Expression> for &'a Atom {
    type Error = &'static str;

    fn try_from(value: &'a Expression) -> Result<Self, Self::Error> {
        match value {
            Expression::Atomic(_, atom) => Ok(atom),
            _ => Err("Cannot convert non-atomic expression to Atom"),
        }
    }
}

impl TryFrom<Box<Expression>> for Atom {
    type Error = &'static str;

    fn try_from(value: Box<Expression>) -> Result<Self, Self::Error> {
        (*value).try_into()
    }
}

impl<'a> TryFrom<&'a Box<Expression>> for &'a Atom {
    type Error = &'static str;

    fn try_from(value: &'a Box<Expression>) -> Result<Self, Self::Error> {
        let expr: &'a Expression = value.borrow();
        expr.try_into()
    }
}

impl TryFrom<Atom> for Literal {
    type Error = &'static str;

    fn try_from(value: Atom) -> Result<Self, Self::Error> {
        match value {
            Atom::Literal(l) => Ok(l),
            _ => Err("Cannot convert non-literal atom to Literal"),
        }
    }
}

impl<'a> TryFrom<&'a Atom> for &'a Literal {
    type Error = &'static str;

    fn try_from(value: &'a Atom) -> Result<Self, Self::Error> {
        match value {
            Atom::Literal(l) => Ok(l),
            _ => Err("Cannot convert non-literal atom to Literal"),
        }
    }
}

impl TryFrom<Atom> for Name {
    type Error = &'static str;

    fn try_from(value: Atom) -> Result<Self, Self::Error> {
        match value {
            Atom::Reference(n) => Ok(n),
            _ => Err("Cannot convert non-reference atom to Name"),
        }
    }
}

impl<'a> TryFrom<&'a Atom> for &'a Name {
    type Error = &'static str;

    fn try_from(value: &'a Atom) -> Result<Self, Self::Error> {
        match value {
            Atom::Reference(n) => Ok(n),
            _ => Err("Cannot convert non-reference atom to Name"),
        }
    }
}

impl TryFrom<Atom> for i32 {
    type Error = &'static str;

    fn try_from(value: Atom) -> Result<Self, Self::Error> {
        let lit: Literal = value.try_into()?;
        lit.try_into()
    }
}

impl TryFrom<&Atom> for i32 {
    type Error = &'static str;

    fn try_from(value: &Atom) -> Result<Self, Self::Error> {
        let lit: &Literal = value.try_into()?;
        lit.try_into()
    }
}

impl TryFrom<Atom> for bool {
    type Error = &'static str;

    fn try_from(value: Atom) -> Result<Self, Self::Error> {
        let lit: Literal = value.try_into()?;
        lit.try_into()
    }
}

impl TryFrom<&Atom> for bool {
    type Error = &'static str;

    fn try_from(value: &Atom) -> Result<Self, Self::Error> {
        let lit: &Literal = value.try_into()?;
        lit.try_into()
    }
}
