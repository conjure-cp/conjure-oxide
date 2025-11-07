use clap::ValueEnum;
use schemars::JsonSchema;
use serde::{Deserialize, Serialize};
use strum_macros::{Display, EnumIter, EnumString};

/// A collection of theories to use for encoding various CO AST constructs.
#[derive(Debug, Default)]
pub struct TheoryConfig {
    pub ints: IntTheory,
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
