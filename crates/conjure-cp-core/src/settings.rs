use std::{fmt::Display, str::FromStr};

pub use crate::ast::QuantifiedExpander;
pub use crate::solver::SolverFamily;

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

#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash, Default)]
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

#[derive(Clone, Copy, Debug, Default, PartialEq, Eq, Hash)]
pub struct SolverArgs {
    pub timeout_ms: Option<u64>,
}
