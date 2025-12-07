use crate::ast::domains::Int;
use crate::ast::domains::range::Range;
use polyquine::Quine;
use serde::{Deserialize, Serialize};
use std::fmt::{Display, Formatter};

#[derive(Clone, Debug, PartialEq, Eq, Hash, Serialize, Deserialize, Quine)]
#[path_prefix(conjure_cp::ast)]
pub struct MSetAttr<A = Int> {
    pub size: Range<A>,
    pub occurrence: Range<A>,   // Relating to minOccurrence, maxOccurrence
}

impl<A> MSetAttr<A> {
    pub fn new(size: Range<A>, occurrence: Range<A>) -> Self {
        Self { size, occurrence }
    }

    //
    // GIVEN ONLY SIZE ATTRIBUTES
    //
    pub fn new_min_max_size(min: A, max: A) -> Self {
        Self::new(Range::Bounded(min, max), Range::Unbounded)
    }

    pub fn new_min_size(min: A) -> Self {
        Self::new(Range::UnboundedR(min), Range::Unbounded)
    }

    pub fn new_max_size(max: A) -> Self {
        Self::new(Range::UnboundedL(max), Range::Unbounded)
    }

    pub fn new_size(sz: A) -> Self {
        Self::new(Range::Single(sz), Range::Unbounded)
    } 



    //
    // GIVEN SIZE AND OCCURRENCE
    //
    pub fn new_min_max_size_min_max_occurrence(min_s: A, max_s: A, min_o: A, max_o: A) -> Self {
        Self::new(Range::Bounded(min_s, max_s), Range::Bounded(min_o, max_o))
    }

    pub fn new_min_size_min_occurrence(min_s: A, min_o: A) -> Self {
        Self::new(Range::UnboundedR(min_s), Range::UnboundedR(min_o))
    }

    pub fn new_min_size_max_occurrence(min_s: A, max_o: A) -> Self {
        Self::new(Range::UnboundedR(min_s), Range::UnboundedL(max_o))
    }

    pub fn new_min_size_min_max_occurrence(min_s: A, min_o: A, max_o: A) -> Self {
        Self::new(Range::UnboundedR(min_s), Range::Bounded(min_o, max_o))
    }

    pub fn new_max_size_min_occurrence(max_s: A, min_o: A) -> Self {
        Self::new(Range::UnboundedL(max_s), Range::UnboundedR(min_o))
    }

    pub fn new_max_size_max_occurrence(max_s: A, max_o: A) -> Self {
        Self::new(Range::UnboundedL(max_s), Range::UnboundedL(max_o))
    }

    pub fn new_max_size_min_max_occurrence(max_s: A, min_o: A, max_o: A) -> Self {
        Self::new(Range::UnboundedL(max_s), Range::Bounded(min_o, max_o))
    }

    pub fn new_size_and_occurrence(sz: A, oc: A) -> Self {
        Self::new(Range::Single(sz), Range::Single(oc))
    }     
}

impl<A> Default for MSetAttr<A> {
    fn default() -> Self {
        MSetAttr {
            size: Range::Unbounded,
            occurrence: Range::Unbounded,
        }
    }
}

impl<A: Display> Display for MSetAttr<A> {
    fn fmt(&self, f: &mut Formatter<'_>) -> std::fmt::Result {
        match &self.size {
            Range::Single(x) => write!(f, "(size {x})"),
            Range::Bounded(l, r) => write!(f, "(minSize {l}, maxSize {r})"),
            Range::UnboundedL(r) => write!(f, "(maxSize {r})"),
            Range::UnboundedR(l) => write!(f, "(minSize {l})"),
            Range::Unbounded => write!(f, ""),
        }
    }

    // TODO @cc398: add for occurrence?
}
