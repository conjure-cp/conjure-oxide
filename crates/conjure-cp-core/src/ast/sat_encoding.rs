use polyquine::Quine;
use serde::{Deserialize, Serialize};
use uniplate::Uniplate;

#[derive(Clone, Debug, PartialEq, Eq, Hash, Serialize, Deserialize, Uniplate, Quine)]
pub enum SATIntEncoding {
    Log,
    Order,
    Direct,
}
