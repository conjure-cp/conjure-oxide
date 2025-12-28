use schemars::JsonSchema;
use serde::{Deserialize, Serialize};
use strum_macros::{Display, EnumIter, EnumString};

#[derive(Debug, PartialEq, Eq, Hash, Clone, Copy, Serialize, Deserialize, JsonSchema, Default)]
pub struct SatConf {
    pub integer_encoding_variant: SatIntType,
    pub solver_variant: SatSolverType,
}

/// There are three different encoding types currently supported by conjure-oxide for using
/// integral variables with a Boolean Satisfiability Solver (Read the Seection on `Sat Encodings'
/// in the book produced during the conjure oxide project)
#[derive(Debug, PartialEq, Eq, Hash, Clone, Copy, Serialize, Deserialize, JsonSchema, Default)]
pub enum SatIntType {
    /// Encoding Integers directly using an n-length array of auxiliary boolean variables
    Direct,
    /// Similar to Order, but the variables are encoded in a different configurations
    Order,
    /// Encoding integers using a `Bit Vector', or a vector of auxiliary boolean decision
    /// variables, which together form the Binary representation of the integer
    #[default]
    Bv,
}

/// RustSAT, which is used for the Sat Solver support, offers support for different sat solvers
#[derive(Debug, PartialEq, Eq, Hash, Clone, Copy, Serialize, Deserialize, JsonSchema, Default)]
pub enum SatSolverType {
    #[default]
    Minisat,
    Kissat,
    CaDiCal,
}
