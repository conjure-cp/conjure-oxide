use serde::{Deserialize, Serialize};

#[derive(Clone, Debug, PartialEq, Eq, Deserialize, Serialize)]
pub enum ReturnType {
    Int,
    Bool,
    Matrix(Box<ReturnType>),
    Set(Box<ReturnType>),
}

/// Something with a return type
pub trait Typeable {
    fn return_type(&self) -> Option<ReturnType>;
}
