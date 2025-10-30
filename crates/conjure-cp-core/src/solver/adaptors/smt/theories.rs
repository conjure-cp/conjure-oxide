use clap::ValueEnum;
use schemars::JsonSchema;
use serde::{Deserialize, Serialize};
use strum_macros::{Display, EnumIter, EnumString};

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
