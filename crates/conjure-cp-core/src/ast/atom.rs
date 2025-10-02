use crate::{ast::declaration::serde::DeclarationPtrAsId, bug};
use std::{borrow::Borrow, cell::Ref};
use uniplate::Uniplate;

use super::{
    AbstractLiteral, DeclarationPtr, Domain, Expression, Literal, Moo, Name,
    categories::{Category, CategoryOf},
    domains::HasDomain,
    records::RecordValue,
};
use derivative::Derivative;
use polyquine::Quine;
use serde::{Deserialize, Serialize};
use serde_with::serde_as;

/// An `Atom` is an indivisible expression, such as a literal or a reference.
#[serde_as]
#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize, Uniplate, Derivative, Quine)]
#[derivative(Hash)]
#[uniplate()]
#[biplate(to=Literal)]
#[biplate(to=Expression)]
#[biplate(to=AbstractLiteral<Literal>)]
#[biplate(to=RecordValue<Literal>)]
#[biplate(to=DeclarationPtr)]
#[biplate(to=Name)]
#[path_prefix(conjure_cp::ast)]
pub enum Atom {
    Literal(Literal),
    // FIXME: check if these are the hashing semantics we want.
    #[polyquine_skip]
    Reference(#[serde_as(as = "DeclarationPtrAsId")] DeclarationPtr),
}

impl Atom {
    pub fn new_ref(decl: DeclarationPtr) -> Atom {
        Atom::Reference(decl)
    }

    pub fn into_declaration(self) -> DeclarationPtr {
        match self {
            Atom::Reference(decl) => decl,
            _ => panic!("Called into_declaration on a non-reference Atom"),
        }
    }
}

impl CategoryOf for Atom {
    fn category_of(&self) -> Category {
        match self {
            Atom::Literal(_) => Category::Constant,
            Atom::Reference(declaration_ptr) => declaration_ptr.category_of(),
        }
    }
}

impl HasDomain for Atom {
    fn domain_of(&self) -> Domain {
        match self {
            Atom::Literal(literal) => literal.domain_of(),
            Atom::Reference(ptr) => ptr.domain().unwrap_or_else(|| {
                bug!("reference ({name}) should have a domain", name = ptr.name())
            }),
        }
    }
}

impl std::fmt::Display for Atom {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            Atom::Literal(x) => x.fmt(f),
            Atom::Reference(x) => x.name().fmt(f),
        }
    }
}

impl From<Literal> for Atom {
    fn from(value: Literal) -> Self {
        Atom::Literal(value)
    }
}

impl From<DeclarationPtr> for Atom {
    fn from(value: DeclarationPtr) -> Self {
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

impl TryFrom<Box<Expression>> for Atom {
    type Error = &'static str;

    fn try_from(value: Box<Expression>) -> Result<Self, Self::Error> {
        TryFrom::try_from(*value)
    }
}

impl TryFrom<Moo<Expression>> for Atom {
    type Error = &'static str;

    fn try_from(value: Moo<Expression>) -> Result<Self, Self::Error> {
        TryFrom::try_from(Moo::unwrap_or_clone(value))
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

impl<'a> TryFrom<&'a Box<Expression>> for &'a Atom {
    type Error = &'static str;

    fn try_from(value: &'a Box<Expression>) -> Result<Self, Self::Error> {
        let expr: &'a Expression = value.borrow();
        expr.try_into()
    }
}

impl<'a> TryFrom<&'a Moo<Expression>> for &'a Atom {
    type Error = &'static str;

    fn try_from(value: &'a Moo<Expression>) -> Result<Self, Self::Error> {
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
            Atom::Reference(x) => Ok(x.name().clone()),
            _ => Err("Cannot convert non-reference atom to Name"),
        }
    }
}

impl<'a> TryFrom<&'a Atom> for Ref<'a, Name> {
    type Error = &'static str;

    fn try_from(value: &'a Atom) -> Result<Self, Self::Error> {
        match value {
            Atom::Reference(x) => Ok(x.name()),
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

impl TryFrom<&Box<Atom>> for i32 {
    type Error = &'static str;

    fn try_from(value: &Box<Atom>) -> Result<Self, Self::Error> {
        TryFrom::<&Atom>::try_from(value.as_ref())
    }
}

impl TryFrom<Box<Atom>> for i32 {
    type Error = &'static str;

    fn try_from(value: Box<Atom>) -> Result<Self, Self::Error> {
        let lit: Literal = (*value).try_into()?;
        lit.try_into()
    }
}

impl TryFrom<&Moo<Atom>> for i32 {
    type Error = &'static str;

    fn try_from(value: &Moo<Atom>) -> Result<Self, Self::Error> {
        TryFrom::<&Atom>::try_from(value.as_ref())
    }
}

impl TryFrom<Moo<Atom>> for i32 {
    type Error = &'static str;

    fn try_from(value: Moo<Atom>) -> Result<Self, Self::Error> {
        TryFrom::<&Atom>::try_from(value.as_ref())
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
