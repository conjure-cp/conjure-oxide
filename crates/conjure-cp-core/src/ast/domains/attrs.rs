use crate::ast::domains::Int;
use crate::ast::domains::range::Range;
use funcmap::{FuncMap, TryFuncMap};
use itertools::Itertools;
use polyquine::Quine;
use serde::{Deserialize, Serialize};
use std::fmt::{Display, Formatter};

#[derive(Clone, Debug, PartialEq, Eq, Hash, Serialize, Deserialize, FuncMap, TryFuncMap, Quine)]
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
        match self.size {
            Range::Unbounded => Ok(()),
            _ => write!(f, "({})", fmt_size("size", &self.size)),
        }
    }
}

#[derive(Clone, Debug, PartialEq, Eq, Hash, Serialize, Deserialize, FuncMap, TryFuncMap, Quine)]
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
        let size_str = fmt_size("size", &self.size);

        // It only makes sense in terms of min and max occurrence for the essence language,
        // so for single ranges it is still presented as min and max occurrence.
        let occ_str = match &self.occurrence {
            Range::Single(x) => format!("minOccur({x}), maxOccur({x})"),
            Range::Bounded(l, r) => format!("minOccur({l}), maxOccur({r})"),
            Range::UnboundedL(r) => format!("maxOccur({r})"),
            Range::UnboundedR(l) => format!("minOccur({l})"),
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

#[derive(Clone, Debug, PartialEq, Eq, Hash, Serialize, Deserialize, FuncMap, TryFuncMap, Quine)]
#[path_prefix(conjure_cp::ast)]
pub struct FuncAttr<A = Int> {
    pub size: Range<A>,
    pub partiality: PartialityAttr,
    pub jectivity: JectivityAttr,
}

impl<A: Display> Display for FuncAttr<A> {
    fn fmt(&self, f: &mut Formatter<'_>) -> std::fmt::Result {
        let size_str = fmt_size("size", &self.size);
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

#[derive(Clone, Debug, PartialEq, Eq, Hash, Serialize, Deserialize, FuncMap, TryFuncMap, Quine)]
#[path_prefix(conjure_cp::ast)]
pub struct SequenceAttr<A = Int> {
    pub size: Range<A>,
    pub jectivity: JectivityAttr,
}

impl<A> Default for SequenceAttr<A> {
    fn default() -> Self {
        SequenceAttr {
            size: Range::Unbounded,
            jectivity: JectivityAttr::None,
        }
    }
}

impl<A: Display> Display for SequenceAttr<A> {
    fn fmt(&self, f: &mut Formatter<'_>) -> std::fmt::Result {
        let size_str = fmt_size("size", &self.size);
        let mut strs = [size_str, self.jectivity.to_string()]
            .iter()
            .filter(|s| !s.is_empty())
            .join(", ");
        if !strs.is_empty() {
            strs = format!("({})", strs);
        }
        write!(f, "{strs}")
    }
}

#[derive(Clone, Debug, PartialEq, Eq, Hash, Serialize, Deserialize, FuncMap, TryFuncMap, Quine)]
#[path_prefix(conjure_cp::ast)]
pub struct PartitionAttr<A = Int> {
    pub num_parts: Range<A>, // i.e. how many parts there are in the partition
    pub part_len: Range<A>,  // i.e. the size of each constitutent part
    pub is_regular: bool,
}

impl<A: Display> Display for PartitionAttr<A> {
    fn fmt(&self, f: &mut Formatter<'_>) -> std::fmt::Result {
        let num_parts_str = fmt_size("numParts", &self.num_parts);
        let part_len_str = fmt_size("partSize", &self.part_len);

        let regular_str = match &self.is_regular {
            true => "regular".to_string(),
            false => String::new(),
        };

        let mut strs = [num_parts_str, part_len_str, regular_str]
            .iter()
            .filter(|s| !s.is_empty())
            .join(", ");
        if !strs.is_empty() {
            strs = format!("({})", strs);
        }
        write!(f, "{strs}")
    }
}

impl<A> Default for PartitionAttr<A> {
    fn default() -> Self {
        PartitionAttr {
            num_parts: Range::Unbounded,
            part_len: Range::Unbounded,
            is_regular: false,
        }
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

#[derive(Clone, Debug, PartialEq, Eq, Hash, Serialize, Deserialize, FuncMap, TryFuncMap, Quine)]
#[path_prefix(conjure_cp::ast)]
pub struct RelAttr<A = Int> {
    pub size: Range<A>,
    pub binary: Vec<BinaryAttr>,
}

impl<A> Default for RelAttr<A> {
    fn default() -> Self {
        RelAttr {
            size: Range::Unbounded,
            binary: Vec::new(),
        }
    }
}

impl<A: Display> Display for RelAttr<A> {
    fn fmt(&self, f: &mut Formatter<'_>) -> std::fmt::Result {
        let size_str = fmt_size("size", &self.size);
        let mut strs = [size_str, self.binary.iter().join(", ")]
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
pub enum BinaryAttr {
    Reflexive,
    Irreflexive,
    Coreflexive,
    Symmetric,
    AntiSymmetric,
    ASymmetric,
    Transitive,
    Total,
    Connex,
    Euclidean,
    Serial,
    Equivalence,
    PartialOrder,
}

impl Display for BinaryAttr {
    fn fmt(&self, f: &mut Formatter<'_>) -> std::fmt::Result {
        match self {
            BinaryAttr::Reflexive => write!(f, "reflexive"),
            BinaryAttr::Irreflexive => write!(f, "irreflexive"),
            BinaryAttr::Coreflexive => write!(f, "coreflexive"),
            BinaryAttr::Symmetric => write!(f, "symmetric"),
            BinaryAttr::AntiSymmetric => write!(f, "antiSymmetric"),
            BinaryAttr::ASymmetric => write!(f, "aSymmetric"),
            BinaryAttr::Transitive => write!(f, "transitive"),
            BinaryAttr::Total => write!(f, "total"),
            BinaryAttr::Connex => write!(f, "connex"),
            BinaryAttr::Euclidean => write!(f, "Euclidean"),
            BinaryAttr::Serial => write!(f, "serial"),
            BinaryAttr::Equivalence => write!(f, "equivalence"),
            BinaryAttr::PartialOrder => write!(f, "partialOrder"),
        }
    }
}

/// Format a range as Essence size attribute
#[inline]
fn fmt_size<A: Display>(suffix: &str, sz: &Range<A>) -> String {
    let cap_suffix = capitalize(suffix);
    match sz {
        Range::Single(x) => format!("{suffix} {x}"),
        Range::Bounded(l, r) => format!("min{cap_suffix} {l}, max{cap_suffix} {r}"),
        Range::UnboundedL(r) => format!("max{cap_suffix} {r}"),
        Range::UnboundedR(l) => format!("min{cap_suffix} {l}"),
        Range::Unbounded => "".to_string(),
    }
}

#[inline]
fn capitalize(s: &str) -> String {
    let mut c = s.chars();
    match c.next() {
        None => String::new(),
        Some(f) => f.to_uppercase().to_string() + c.as_str(),
    }
}
