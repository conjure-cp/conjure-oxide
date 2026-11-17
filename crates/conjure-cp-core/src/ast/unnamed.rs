use polyquine::Quine;
use serde::{Deserialize, Serialize};

use crate::ast::{Moo, Name};

/// A finite nominal type whose members have no names in the input model.
#[derive(Clone, Debug, PartialEq, Eq, Hash, Serialize, Deserialize, Quine)]
pub struct UnnamedType {
    pub name: Name,
    pub size: u32,
}

/// Members are numbered from one for representation and solution output.
#[derive(Clone, Debug, PartialEq, Eq, Hash, Serialize, Deserialize, Quine)]
pub struct UnnamedValue {
    pub ty: Moo<UnnamedType>,
    pub value: u32,
}
