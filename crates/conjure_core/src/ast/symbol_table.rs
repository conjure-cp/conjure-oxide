use std::collections::BTreeMap;
use std::fmt::Display;

use serde::{Deserialize, Serialize};

use crate::ast::variables::DecisionVariable;

#[derive(Clone, Debug, Eq, PartialEq, Ord, PartialOrd, Hash, Serialize, Deserialize)]
pub enum Name {
    UserName(String),
    MachineName(i32),
}

uniplate::derive_unplateable!(Name);

impl Display for Name {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            Name::UserName(s) => write!(f, "{}", s),
            Name::MachineName(i) => write!(f, "__{}", i),
        }
    }
}

pub type SymbolTable = BTreeMap<Name, DecisionVariable>;
