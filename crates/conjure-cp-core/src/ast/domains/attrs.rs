use crate::ast::domains::Int;
use crate::ast::domains::range::Range;
use polyquine::Quine;
use serde::{Deserialize, Serialize};
use std::fmt::{Display, Formatter};

#[derive(Clone, Debug, PartialEq, Eq, Hash, Serialize, Deserialize, Quine)]
#[path_prefix(conjure_cp::ast)]
pub struct SetAttr<A = Int> {
    pub size: Range<A>,
}

impl<A> SetAttr<A> {
    pub fn new(size: Range<A>) -> Self {
        Self { size }
    }

    pub fn new_min_max_size(min: A, max: A) -> Self {
        Self::new(Range::Bounded(min, max))
    }

    pub fn new_min_size(min: A) -> Self {
        Self::new(Range::UnboundedR(min))
    }

    pub fn new_max_size(max: A) -> Self {
        Self::new(Range::UnboundedL(max))
    }

    pub fn new_size(sz: A) -> Self {
        Self::new(Range::Single(sz))
    }
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

#[derive(Clone, Debug, PartialEq, Eq, Hash, Serialize, Deserialize, Quine)]
#[path_prefix(conjure_cp::ast)]
pub struct FuncAttr<A = Int> {
    pub size: Range<A>,
    pub partiality: PartialityAttr,
    pub jectivity: JectivityAttr,
}

impl<A: Display> Display for FuncAttr<A> {
    fn fmt(&self, f: &mut Formatter<'_>) -> std::fmt::Result {
        write!(f, "({}{}{})", self.size, self.partiality, self.jectivity)
    }
}

#[derive(Clone, Debug, PartialEq, Eq, Hash, Serialize, Deserialize, Quine)]
pub enum PartialityAttr {
    Total,
    Partial,
}

impl Display for PartialityAttr {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            PartialityAttr::Total => write!(f, " total"),
            PartialityAttr::Partial => write!(f, " partial"),
        }
    }
}

#[derive(Clone, Debug, PartialEq, Eq, Hash, Serialize, Deserialize, Quine)]
pub enum JectivityAttr {
    None,
    Injective,
    Surjective,
    Bijective,
}

impl Display for JectivityAttr {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            JectivityAttr::None => write!(f, ""),
            JectivityAttr::Injective => write!(f, " injective"),
            JectivityAttr::Surjective => write!(f, " surjective"),
            JectivityAttr::Bijective => write!(f, " bijective"),
        }
    }
}
