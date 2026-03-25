use polyquine::Quine;
use serde::{Deserialize, Serialize};

use crate::ast::Name;

#[derive(Clone, Debug, PartialEq, Eq, Hash, Serialize, Deserialize, Quine)]
pub struct EnumeratedType {
    pub variants: Vec<Name>,
}
