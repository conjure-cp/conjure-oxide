use std::{fmt::Display, str::FromStr};

use schemars::JsonSchema;
use serde::{Deserialize, Serialize};
use strum_macros::{Display as StrumDisplay, EnumIter};

#[cfg(feature = "smt")]
use crate::solver::adaptors::smt::{IntTheory, MatrixTheory, TheoryConfig};

#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash, Default)]
pub enum Parser {
    #[default]
    TreeSitter,
    ViaConjure,
}

impl Display for Parser {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            Parser::TreeSitter => write!(f, "tree-sitter"),
            Parser::ViaConjure => write!(f, "via-conjure"),
        }
    }
}

impl FromStr for Parser {
    type Err = String;

    fn from_str(s: &str) -> Result<Self, Self::Err> {
        match s.trim().to_ascii_lowercase().as_str() {
            "tree-sitter" => Ok(Parser::TreeSitter),
            "via-conjure" => Ok(Parser::ViaConjure),
            other => Err(format!(
                "unknown parser: {other}; expected one of: tree-sitter, via-conjure"
            )),
        }
    }
}

#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash)]
pub enum Rewriter {
    Naive,
    Morph,
}

impl Display for Rewriter {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            Rewriter::Naive => write!(f, "naive"),
            Rewriter::Morph => write!(f, "morph"),
        }
    }
}

impl FromStr for Rewriter {
    type Err = String;

    fn from_str(s: &str) -> Result<Self, Self::Err> {
        match s.trim().to_ascii_lowercase().as_str() {
            "naive" => Ok(Rewriter::Naive),
            "morph" => Ok(Rewriter::Morph),
            other => Err(format!(
                "unknown rewriter: {other}; expected one of: naive, morph"
            )),
        }
    }
}

#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash)]
pub enum QuantifiedExpander {
    ExpandNative,
    ExpandViaSolver,
    ExpandViaSolverAc,
}

impl Display for QuantifiedExpander {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            QuantifiedExpander::ExpandNative => write!(f, "native"),
            QuantifiedExpander::ExpandViaSolver => write!(f, "via-solver"),
            QuantifiedExpander::ExpandViaSolverAc => write!(f, "via-solver-ac"),
        }
    }
}

impl FromStr for QuantifiedExpander {
    type Err = String;

    fn from_str(s: &str) -> Result<Self, Self::Err> {
        match s.trim().to_ascii_lowercase().as_str() {
            "native" => Ok(QuantifiedExpander::ExpandNative),
            "via-solver" => Ok(QuantifiedExpander::ExpandViaSolver),
            "via-solver-ac" => Ok(QuantifiedExpander::ExpandViaSolverAc),
            _ => Err(format!(
                "unknown quantified expander: {s}; expected one of: \
                 native, via-solver, via-solver-ac"
            )),
        }
    }
}

impl QuantifiedExpander {
    pub(crate) const fn as_u8(self) -> u8 {
        match self {
            QuantifiedExpander::ExpandNative => 0,
            QuantifiedExpander::ExpandViaSolver => 1,
            QuantifiedExpander::ExpandViaSolverAc => 2,
        }
    }

    pub(crate) const fn from_u8(value: u8) -> Self {
        match value {
            0 => QuantifiedExpander::ExpandNative,
            1 => QuantifiedExpander::ExpandViaSolver,
            2 => QuantifiedExpander::ExpandViaSolverAc,
            _ => QuantifiedExpander::ExpandNative,
        }
    }
}

#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash, Default, Serialize, Deserialize, JsonSchema)]
pub enum SatEncoding {
    #[default]
    Log,
    Direct,
    Order,
}

impl SatEncoding {
    pub const fn as_rule_set(self) -> &'static str {
        match self {
            SatEncoding::Log => "SAT_Log",
            SatEncoding::Direct => "SAT_Direct",
            SatEncoding::Order => "SAT_Order",
        }
    }
}

impl Display for SatEncoding {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            SatEncoding::Log => write!(f, "log"),
            SatEncoding::Direct => write!(f, "direct"),
            SatEncoding::Order => write!(f, "order"),
        }
    }
}

impl FromStr for SatEncoding {
    type Err = String;

    fn from_str(s: &str) -> Result<Self, Self::Err> {
        match s.trim().to_ascii_lowercase().as_str() {
            "log" => Ok(SatEncoding::Log),
            "direct" => Ok(SatEncoding::Direct),
            "order" => Ok(SatEncoding::Order),
            other => Err(format!(
                "unknown sat-encoding: {other}; expected one of: log, direct, order"
            )),
        }
    }
}

#[derive(
    Debug,
    EnumIter,
    StrumDisplay,
    PartialEq,
    Eq,
    Hash,
    Clone,
    Copy,
    Serialize,
    Deserialize,
    JsonSchema,
)]
pub enum SolverFamily {
    Minion,
    Sat(SatEncoding),
    #[cfg(feature = "smt")]
    Smt(TheoryConfig),
}

impl FromStr for SolverFamily {
    type Err = String;

    fn from_str(s: &str) -> Result<Self, Self::Err> {
        let s = s.trim().to_ascii_lowercase();

        match s.as_str() {
            "minion" => Ok(SolverFamily::Minion),
            "sat" | "sat-log" => Ok(SolverFamily::Sat(SatEncoding::Log)),
            "sat-direct" => Ok(SolverFamily::Sat(SatEncoding::Direct)),
            "sat-order" => Ok(SolverFamily::Sat(SatEncoding::Order)),
            #[cfg(feature = "smt")]
            "smt" => Ok(SolverFamily::Smt(TheoryConfig::default())),
            other => {
                // allow forms like `smt-bv-atomic` or `smt-lia-arrays`
                #[cfg(feature = "smt")]
                if other.starts_with("smt-") {
                    let parts = other.split('-').skip(1);
                    let mut ints = IntTheory::default();
                    let mut matrices = MatrixTheory::default();
                    let mut unwrap_alldiff = false;

                    for token in parts {
                        match token {
                            "" => {}
                            "lia" => ints = IntTheory::Lia,
                            "bv" => ints = IntTheory::Bv,
                            "arrays" => matrices = MatrixTheory::Arrays,
                            "atomic" => matrices = MatrixTheory::Atomic,
                            "nodiscrete" => unwrap_alldiff = true,
                            other_token => {
                                return Err(format!(
                                    "unknown SMT theory option '{other_token}', must be one of bv|lia|arrays|atomic|nodiscrete"
                                ));
                            }
                        }
                    }

                    return Ok(SolverFamily::Smt(TheoryConfig {
                        ints,
                        matrices,
                        unwrap_alldiff,
                    }));
                }
                Err(format!(
                    "unknown solver family '{other}', expected one of: minion, sat-log, sat-direct, sat-order, smt[(bv|lia)-(arrays|atomic)][-nodiscrete]"
                ))
            }
        }
    }
}

impl SolverFamily {
    pub const fn as_str(&self) -> &'static str {
        match self {
            SolverFamily::Minion => "minion",
            SolverFamily::Sat(_) => "sat",
            #[cfg(feature = "smt")]
            SolverFamily::Smt(_) => "smt",
        }
    }
}

#[derive(Clone, Copy, Debug, Default, PartialEq, Eq, Hash)]
pub struct SolverArgs {
    pub timeout_ms: Option<u64>,
}
