use std::fmt::Display;

use polyquine::Quine;
use serde::{Deserialize, Serialize};

use crate::ast::{Domain, DomainPtr, Moo, Name, Range};

#[derive(Clone, Debug, PartialEq, Eq, Hash, Serialize, Deserialize, Quine)]
pub struct EnumeratedType {
    pub name: Name,
    pub variants: Vec<Name>,
}

impl EnumeratedType {
    pub fn new(name: Name, variants: impl Into<Vec<Name>>) -> Self {
        Self {
            name,
            variants: variants.into(),
        }
    }

    pub fn len(&self) -> usize {
        self.variants.len() as usize
    }
}

impl Moo<EnumeratedType> {
    pub fn to_domain(&self) -> DomainPtr {
        Domain::enumerated_ground(self.clone(), vec![Range::Unbounded])
    }
}

impl Display for EnumeratedType {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        let name = &self.name;
        write!(f, "enum {name}")
    }
}

#[derive(Clone, Debug, PartialEq, Eq, Hash, Serialize, Deserialize, Quine)]
pub struct EnumVariant {
    pub ty: Moo<EnumeratedType>,
    pub variant: u32,
}

impl EnumVariant {
    pub fn new(ty: Moo<EnumeratedType>, variant: u32) -> Self {
        Self { ty, variant }
    }

    pub fn name(&self) -> &Name {
        &self.ty.variants[self.variant as usize]
    }

    pub fn pred(&self) -> Option<EnumVariant> {
        if self.variant == 0 {
            return None;
        }

        Some(Self::new(self.ty.clone(), self.variant - 1))
    }

    pub fn succ(&self) -> Option<EnumVariant> {
        if self.variant == self.ty.variants.len() as u32 {
            return None;
        }

        Some(Self::new(self.ty.clone(), self.variant + 1))
    }
}
