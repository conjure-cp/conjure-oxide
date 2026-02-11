use crate::ast::domains::Int;
use crate::ast::domains::range::Range;
use itertools::Itertools;
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
            Range::Single(x) => write!(f, "(size({x}))"),
            Range::Bounded(l, r) => write!(f, "(minSize({l}), maxSize({r}))"),
            Range::UnboundedL(r) => write!(f, "(maxSize({r}))"),
            Range::UnboundedR(l) => write!(f, "(minSize({l}))"),
            Range::Unbounded => write!(f, ""),
        }
    }
}

#[derive(Clone, Debug, PartialEq, Eq, Hash, Serialize, Deserialize, Quine)]
#[path_prefix(conjure_cp::ast)]
pub struct MSetAttr<A = Int> {
    pub size: Range<A>,
    pub occurrence: Range<A>,
}

impl<A> MSetAttr<A> {
    pub fn new(size: Range<A>, occurrence: Range<A>) -> Self {
        Self { size, occurrence }
    }

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
}

impl<A: Display> Display for MSetAttr<A> {
    fn fmt(&self, f: &mut Formatter<'_>) -> std::fmt::Result {
        let size_str = match &self.size {
            Range::Single(x) => format!("size {x} "),
            Range::Bounded(l, r) => format!("minSize {l} , maxSize {r} "),
            Range::UnboundedL(r) => format!("maxSize {r} "),
            Range::UnboundedR(l) => format!("minSize {l} "),
            Range::Unbounded => "".to_string(),
        };

        // It only makes sense in terms of min and max occurrence for the essence language,
        // so for single ranges it is still presented as min and max occurrence.
        let occ_str = match &self.occurrence {
            Range::Single(x) => format!("minOccur {x}, maxOccur {x}"),
            Range::Bounded(l, r) => format!("minOccur {l} , maxOccur {r} "),
            Range::UnboundedL(r) => format!("maxOccur {r} "),
            Range::UnboundedR(l) => format!("minOccur {l} "),
            Range::Unbounded => "".to_string(),
        };

        let mut strs = [size_str, occ_str]
            .iter()
            .filter(|s| !s.is_empty())
            .join(", ");
        if !strs.is_empty() {
            strs = format!("({})", strs);
        }
        write!(f, "{strs}")
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

#[derive(Clone, Debug, PartialEq, Eq, Hash, Serialize, Deserialize, Quine)]
#[path_prefix(conjure_cp::ast)]
pub struct FuncAttr<A = Int> {
    pub size: Range<A>,
    pub partiality: PartialityAttr,
    pub jectivity: JectivityAttr,
}

impl<A: Display> Display for FuncAttr<A> {
    fn fmt(&self, f: &mut Formatter<'_>) -> std::fmt::Result {
        let size_str = match &self.size {
            Range::Single(x) => format!("size({x})"),
            Range::Bounded(l, r) => format!("minSize({l}), maxSize({r})"),
            Range::UnboundedL(r) => format!("maxSize({r})"),
            Range::UnboundedR(l) => format!("minSize({l})"),
            Range::Unbounded => "".to_string(),
        };
        let mut strs = [
            size_str,
            self.partiality.to_string(),
            self.jectivity.to_string(),
        ]
        .iter()
        .filter(|s| !s.is_empty())
        .join(", ");
        if !strs.is_empty() {
            strs = format!("({})", strs);
        }
        write!(f, "{strs}")
    }
}

#[derive(Clone, Debug, PartialEq, Eq, Hash, Serialize, Deserialize, Quine)]
pub enum PartialityAttr {
    Total,
    Partial,
}

impl Display for PartialityAttr {
    fn fmt(&self, f: &mut Formatter<'_>) -> std::fmt::Result {
        match self {
            PartialityAttr::Total => write!(f, "total"),
            PartialityAttr::Partial => write!(f, ""),
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
    fn fmt(&self, f: &mut Formatter<'_>) -> std::fmt::Result {
        match self {
            JectivityAttr::None => write!(f, ""),
            JectivityAttr::Injective => write!(f, "injective"),
            JectivityAttr::Surjective => write!(f, "surjective"),
            JectivityAttr::Bijective => write!(f, "bijective"),
        }
    }
}
