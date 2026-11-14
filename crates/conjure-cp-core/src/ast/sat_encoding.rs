use polyquine::Quine;
use serde::{Deserialize, Serialize};
use uniplate::Uniplate;

#[derive(Clone, Debug, PartialEq, Eq, Hash, Serialize, Deserialize, Uniplate, Quine)]
pub enum SATIntEncoding {
    Log,
    /// Unsigned displacement from the domain minimum.
    Offset,
    /// Unsigned index into the canonical inclusive domain intervals.
    Rank(Vec<(i32, i32)>),
    /// Magnitude bits, least significant first, then a sign bit.
    SignMagnitude,
    Order,
    Direct,
}
