use std::borrow::Borrow;
use std::cell::RefCell;
use std::rc::Rc;

use super::{
    literals::AbstractLiteral, records::RecordValue, Declaration, Expression, Literal, Name,
    ReturnType, Typeable,
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
    Reference(Name, #[serde(skip)] Rc<RefCell<Declaration>>),
}

impl Atom {
    pub fn new_ref(decl: &Declaration) -> Atom {
        Atom::Reference(decl.name().clone(), Rc::new(RefCell::new(decl.clone())))
    }

    pub fn into_declaration(self) -> Rc<RefCell<Declaration>> {
        match self {
            Atom::Reference(_, decl) => decl.clone(),
            _ => panic!("Called into_declaration on a non-reference Atom"),
        }
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
            Atom::Reference(_, _) => None,
        }
    }
}

impl std::fmt::Display for Atom {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            Atom::Literal(x) => x.fmt(f),
            Atom::Reference(x, _) => x.fmt(f),
        }
    }
}

impl From<Literal> for Atom {
    fn from(value: Literal) -> Self {
        Atom::Literal(value)
    }
}

impl From<(Name, Rc<RefCell<Declaration>>)> for Atom {
    fn from((name, decl): (Name, Rc<RefCell<Declaration>>)) -> Self {
        Atom::Reference(name, decl)
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

impl From<Declaration> for Atom {
    fn from(decl: Declaration) -> Self {
        // Clone the name from the declaration
        let name = decl.name().clone();
        // Wrap the declaration in Rc<RefCell<>>
        let decl_rc = Rc::new(RefCell::new(decl));
        // Create the Atom::Reference
        Atom::Reference(name, decl_rc)
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
            Atom::Reference(n, _) => Ok(n),
            _ => Err("Cannot convert non-reference atom to Name"),
        }
    }
}

impl<'a> TryFrom<&'a Atom> for &'a Name {
    type Error = &'static str;

    fn try_from(value: &'a Atom) -> Result<Self, Self::Error> {
        match value {
            Atom::Reference(n, _) => Ok(n),
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
