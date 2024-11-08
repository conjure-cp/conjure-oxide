use std::collections::HashMap;
use std::fmt::Display;

use serde::{Deserialize, Serialize};

use crate::ast::variables::DecisionVariable;
use uniplate::{Biplate, Uniplate};

#[derive(Clone, Debug, Eq, PartialEq, Ord, PartialOrd, Hash, Serialize, Deserialize)]
pub enum Name {
    UserName(String),
    MachineName(i32),
}

uniplate::derive_unplateable!(Name);

impl Display for Name {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            Name::UserName(s) => write!(f, "UserName({})", s),
            Name::MachineName(i) => write!(f, "MachineName({})", i),
        }
    }
}

pub type SymbolTable = HashMap<Name, DecisionVariable>;
