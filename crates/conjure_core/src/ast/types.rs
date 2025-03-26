use derive_to_tokens::ToTokens;
use serde::{Deserialize, Serialize};

#[derive(Clone, Debug, PartialEq, Eq, Deserialize, Serialize, ToTokens)]
pub enum ReturnType {
    Int,
    Bool,
    Matrix(#[to_tokens(recursive)] Box<ReturnType>),
}

/// Something with a return type
pub trait Typeable {
    fn return_type(&self) -> Option<ReturnType>;
}
