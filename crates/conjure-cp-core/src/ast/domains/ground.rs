use crate::ast::pretty::pretty_vec;
use crate::ast::{
    Moo, RecordEntry, SetAttr, Typeable,
    domains::{domain::Int, range::Range},
};
use conjure_cp_core::ast::ReturnType;
use itertools::Itertools;
use polyquine::Quine;
use serde::{Deserialize, Serialize};
use std::fmt::{Display, Formatter};

#[derive(Clone, Debug, PartialEq, Eq, Hash, Serialize, Deserialize, Quine)]
pub enum GroundDomain {
    /// An empty domain of a given type
    Empty(ReturnType),
    /// A boolean value (true / false)
    Bool,
    /// An integer value in the given ranges (e.g. int(1, 3..5))
    Int(Vec<Range<Int>>),
    /// A set of elements drawn from the inner domain
    Set(SetAttr<Int>, Moo<GroundDomain>),
    /// An N-dimensional matrix of elements drawn from the inner domain,
    /// and indices from the n index domains
    Matrix(Moo<GroundDomain>, Vec<GroundDomain>),
    /// A tuple of N elements, each with its own domain
    Tuple(Vec<GroundDomain>),
    Record(Vec<RecordEntry<GroundDomain>>),
}

impl Typeable for GroundDomain {
    fn return_type(&self) -> Option<ReturnType> {
        match self {
            GroundDomain::Empty(ty) => Some(ty.clone()),
            GroundDomain::Bool => Some(ReturnType::Bool),
            GroundDomain::Int(_) => Some(ReturnType::Int),
            GroundDomain::Set(_attr, inner) => {
                inner.return_type().map(|ty| ReturnType::Set(Box::new(ty)))
            }
            GroundDomain::Matrix(inner, _idx) => inner
                .return_type()
                .map(|ty| ReturnType::Matrix(Box::new(ty))),
            GroundDomain::Tuple(inners) => {
                let mut inner_types = Vec::new();
                for inner in inners {
                    inner_types.push(inner.return_type()?);
                }
                Some(ReturnType::Tuple(inner_types))
            }
            GroundDomain::Record(entries) => {
                let mut entry_types = Vec::new();
                for entry in entries {
                    entry_types.push(entry.domain.return_type()?);
                }
                Some(ReturnType::Record(entry_types))
            }
        }
    }
}

impl Display for GroundDomain {
    fn fmt(&self, f: &mut Formatter<'_>) -> std::fmt::Result {
        match &self {
            GroundDomain::Empty(ty) => write!(f, "empty({ty:?})"),
            GroundDomain::Bool => write!(f, "bool"),
            GroundDomain::Int(ranges) => {
                if ranges.iter().all(Range::is_bounded) {
                    let rngs: String = ranges.iter().map(|r| format!("{r}")).join(", ");
                    write!(f, "int({})", rngs)
                } else {
                    write!(f, "int")
                }
            }
            GroundDomain::Set(attrs, inner_dom) => write!(f, "set {attrs} of {inner_dom}"),
            GroundDomain::Matrix(value_domain, index_domains) => {
                write!(
                    f,
                    "matrix indexed by [{}] of {value_domain}",
                    pretty_vec(&index_domains.iter().collect_vec())
                )
            }
            GroundDomain::Tuple(domains) => {
                write!(
                    f,
                    "tuple of ({})",
                    pretty_vec(&domains.iter().collect_vec())
                )
            }
            GroundDomain::Record(entries) => {
                write!(
                    f,
                    "record of ({})",
                    pretty_vec(
                        &entries
                            .iter()
                            .map(|entry| format!("{}: {}", entry.name, entry.domain))
                            .collect_vec()
                    )
                )
            }
        }
    }
}
