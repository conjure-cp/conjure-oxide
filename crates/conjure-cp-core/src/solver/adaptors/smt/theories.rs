use clap::ValueEnum;
use schemars::JsonSchema;
use serde::{Deserialize, Serialize};
use strum_macros::{Display, EnumIter, EnumString};

/// A collection of theories to use for encoding various CO AST constructs.
#[derive(Debug, PartialEq, Eq, Hash, Clone, Copy, Serialize, Deserialize, JsonSchema, Default)]
pub struct TheoryConfig {
    pub ints: IntTheory,
    pub matrices: MatrixTheory,
    pub unwrap_alldiff: bool,
}

impl TheoryConfig {
    pub fn as_str(self) -> String {
        let mut label = format!("{}-{}", self.ints.as_str(), self.matrices.as_str());
        if self.unwrap_alldiff {
            label.push_str("-nodiscrete");
        }
        label
    }
}

/// The theory to use when encoding CO integers through the SMT solver adaptor.
#[derive(
    Debug,
    EnumString,
    EnumIter,
    Display,
    PartialEq,
    Eq,
    Hash,
    Clone,
    Copy,
    Serialize,
    Deserialize,
    JsonSchema,
    ValueEnum,
    Default,
)]
pub enum IntTheory {
    /// Use Z3 Linear Integer Arithmetic theory for integers
    #[default]
    Lia,

    /// Use Z3 Bitvector theory for integers
    Bv,
}

impl IntTheory {
    pub const fn as_str(self) -> &'static str {
        match self {
            IntTheory::Lia => "lia",
            IntTheory::Bv => "bv",
        }
    }
}

/// The theory to use when encoding CO integers through the SMT solver adaptor.
#[derive(
    Debug,
    EnumString,
    EnumIter,
    Display,
    PartialEq,
    Eq,
    Hash,
    Clone,
    Copy,
    Serialize,
    Deserialize,
    JsonSchema,
    ValueEnum,
    Default,
)]
pub enum MatrixTheory {
    /// Directly encode matrices as SMT Arrays
    #[default]
    Arrays,

    /// Decompose arrays into auxiliary variables using the matrix_to_atom representation
    Atomic,
}

impl MatrixTheory {
    pub const fn as_str(self) -> &'static str {
        match self {
            MatrixTheory::Arrays => "arrays",
            MatrixTheory::Atomic => "atomic",
        }
    }
}
