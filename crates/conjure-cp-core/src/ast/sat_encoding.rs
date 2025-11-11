
use uniplate::Uniplate;
use serde::{Deserialize, Serialize};
use polyquine::Quine;

#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize, Uniplate, Quine)]
pub enum SATIntEncoding {
    Log,
    Order,
    Direct
}