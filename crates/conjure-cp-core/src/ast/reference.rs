use crate::ast::declaration::serde::DeclarationPtrAsId;
use crate::ast::serde::HasId;
use crate::{ast::DeclarationPtr, bug};
use derivative::Derivative;
use serde::{Deserialize, Serialize};
use serde_with::serde_as;
use std::fmt::{Display, Formatter};
use uniplate::Uniplate;

use super::{
    Domain, Name,
    categories::{Category, CategoryOf},
    domains::HasDomain,
};

/// A reference to a declaration (variable, parameter, etc.)
#[serde_as]
#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize, Uniplate, Derivative)]
#[derivative(Hash)]
#[uniplate()]
#[biplate(to=DeclarationPtr)]
#[biplate(to=Name)]
pub struct Reference {
    #[serde_as(as = "DeclarationPtrAsId")]
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

    pub fn name(&self) -> std::cell::Ref<'_, Name> {
        self.ptr.name()
    }

    pub fn id(&self) -> crate::ast::serde::ObjId {
        self.ptr.id()
    }

    pub fn domain(&self) -> Option<Domain> {
        self.ptr.domain()
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
    fn domain_of(&self) -> Domain {
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
