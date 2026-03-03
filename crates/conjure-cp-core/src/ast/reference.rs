use crate::ast::serde::{AsId, HasId};
use crate::{ast::DeclarationPtr, bug};
use derivative::Derivative;
use parking_lot::MappedRwLockReadGuard;
use serde::{Deserialize, Serialize};
use serde_with::serde_as;
use std::fmt::{Display, Formatter};
use uniplate::Uniplate;

use super::{
    Atom, DeclarationKind, DomainPtr, Expression, GroundDomain, Literal, Metadata, Moo, Name,
    categories::{Category, CategoryOf},
    domains::HasDomain,
};

/// A reference to a declaration (variable, parameter, etc.)
///
/// This is a thin wrapper around [`DeclarationPtr`] with two main purposes:
/// 1. Encapsulate the serde pragmas (e.g., serializing as IDs rather than full objects)
/// 2. Enable type-directed traversals of references via uniplate
#[serde_as]
#[derive(
    Clone, Debug, PartialEq, Eq, PartialOrd, Ord, Serialize, Deserialize, Uniplate, Derivative,
)]
#[derivative(Hash)]
#[uniplate()]
#[biplate(to=DeclarationPtr)]
#[biplate(to=Name)]
pub struct Reference {
    #[serde_as(as = "AsId")]
    pub ptr: DeclarationPtr,
}

impl Reference {
    pub fn new(ptr: DeclarationPtr) -> Self {
        Reference { ptr }
    }

    pub fn ptr(&self) -> &DeclarationPtr {
        &self.ptr
    }

    pub fn into_ptr(self) -> DeclarationPtr {
        self.ptr
    }

    pub fn name(&self) -> MappedRwLockReadGuard<'_, Name> {
        self.ptr.name()
    }

    pub fn id(&self) -> crate::ast::serde::ObjId {
        self.ptr.id()
    }

    pub fn domain(&self) -> Option<DomainPtr> {
        self.ptr.domain()
    }

    pub fn resolved_domain(&self) -> Option<Moo<GroundDomain>> {
        self.domain()?.resolve()
    }

    /// Returns the expression behind a value-letting reference, if this is one.
    pub fn resolve_expression(&self) -> Option<Expression> {
        if let Some(expr) = self.ptr().as_value_letting() {
            return Some(expr.clone());
        }

        let generator = {
            let kind = self.ptr.kind();
            if let DeclarationKind::Quantified(inner) = &*kind {
                inner.generator().cloned()
            } else {
                None
            }
        };

        if let Some(generator) = generator
            && let Some(expr) = generator.as_value_letting()
        {
            return Some(expr.clone());
        }

        None
    }

    /// Evaluates this reference to a literal if it resolves to a constant.
    pub fn resolve_constant(&self) -> Option<Literal> {
        self.resolve_expression()
            .and_then(|expr| super::eval::eval_constant(&expr))
    }

    /// Resolves this reference to an atomic expression, if possible.
    pub fn resolve_atomic(&self) -> Option<Atom> {
        self.resolve_expression().and_then(|expr| match expr {
            Expression::Atomic(_, atom) => Some(atom),
            _ => None,
        })
    }
}

impl From<Reference> for Expression {
    fn from(value: Reference) -> Self {
        Expression::Atomic(Metadata::new(), value.into())
    }
}

impl From<DeclarationPtr> for Reference {
    fn from(ptr: DeclarationPtr) -> Self {
        Reference::new(ptr)
    }
}

impl CategoryOf for Reference {
    fn category_of(&self) -> Category {
        self.ptr.category_of()
    }
}

impl HasDomain for Reference {
    fn domain_of(&self) -> DomainPtr {
        self.ptr.domain().unwrap_or_else(|| {
            bug!(
                "reference ({name}) should have a domain",
                name = self.ptr.name()
            )
        })
    }
}

impl Display for Reference {
    fn fmt(&self, f: &mut Formatter<'_>) -> std::fmt::Result {
        self.ptr.name().fmt(f)
    }
}
