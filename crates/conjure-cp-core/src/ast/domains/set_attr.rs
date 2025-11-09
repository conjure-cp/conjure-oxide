use crate::ast::domains::range::Range;
use polyquine::Quine;
use serde::{Deserialize, Serialize};
use std::fmt::{Display, Formatter};

#[derive(Clone, Debug, PartialEq, Eq, Hash, Serialize, Deserialize, Quine)]
pub struct SetAttr<A> {
    pub size: Range<A>,
}

impl<A> Default for SetAttr<A> {
    fn default() -> Self {
        SetAttr {
            size: Range::Unbounded,
        }
    }
}

impl<A: Display> Display for SetAttr<A> {
    fn fmt(&self, f: &mut Formatter<'_>) -> std::fmt::Result {
        match &self.size {
            Range::Single(x) => write!(f, "(size {x})"),
            Range::Bounded(l, r) => write!(f, "(minSize {l}, maxSize {r})"),
            Range::UnboundedL(r) => write!(f, "(maxSize {r})"),
            Range::UnboundedR(l) => write!(f, "(minSize {l})"),
            Range::Unbounded => write!(f, ""),
        }
    }
}
