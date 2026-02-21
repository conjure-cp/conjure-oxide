use std::{fmt::Display, str::FromStr};

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
