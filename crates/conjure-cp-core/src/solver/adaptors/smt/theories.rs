use schemars::JsonSchema;
use serde::{Deserialize, Serialize};
use strum_macros::{Display, EnumIter, EnumString};

/// The Z3 theory an integer variable is expressed in.
///
/// This is not a solver-wide setting: it is chosen per declaration by the `lia` and `bv`
/// representations, so one model can hold variables of both kinds, channelled together where they
/// meet. The adaptor reads the choice off each declaration's representation and gives the variable
/// the matching sort.
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
    Default,
)]
pub enum IntTheory {
    /// Z3's linear integer arithmetic: mathematical integers, no wrapping.
    #[default]
    Lia,

    /// Z3's fixed-width bit-vectors: machine words, with wrapping arithmetic.
    Bv,
}

impl IntTheory {
    pub const fn as_str(self) -> &'static str {
        match self {
            IntTheory::Lia => "lia",
            IntTheory::Bv => "bv",
        }
    }

    /// The theory named by a representation's short name, if it names one.
    pub fn from_repr_short_name(short_name: &str) -> Option<Self> {
        match short_name {
            "lia" => Some(IntTheory::Lia),
            "bv" => Some(IntTheory::Bv),
            _ => None,
        }
    }
}
