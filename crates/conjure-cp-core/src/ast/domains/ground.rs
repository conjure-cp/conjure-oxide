use crate::ast::pretty::pretty_vec;
use crate::ast::{
    Domain, DomainOpError, Literal, Moo, RecordEntry, SetAttr, Typeable,
    domains::{domain::Int, range::Range},
};
use conjure_cp_core::ast::{DomainPtr, Name, ReturnType};
use itertools::Itertools;
use polyquine::Quine;
use serde::{Deserialize, Serialize};
use std::fmt::{Display, Formatter};
use std::iter::zip;
use uniplate::Uniplate;

#[derive(Clone, Debug, PartialEq, Eq, Hash, Serialize, Deserialize, Uniplate, Quine)]
#[path_prefix(conjure_cp::ast)]
pub struct RecordEntryGround {
    pub name: Name,
    pub domain: Moo<GroundDomain>,
}

impl Into<RecordEntry> for RecordEntryGround {
    fn into(self) -> RecordEntry {
        RecordEntry {
            name: self.name,
            domain: self.domain.into(),
        }
    }
}

impl TryFrom<RecordEntry> for RecordEntryGround {
    type Error = DomainOpError;

    fn try_from(value: RecordEntry) -> Result<Self, Self::Error> {
        match value.domain.as_ref() {
            Domain::Ground(gd) => Ok(RecordEntryGround {
                name: value.name,
                domain: gd.clone(),
            }),
            Domain::Unresolved(_) => Err(DomainOpError::InputContainsReference),
        }
    }
}

#[derive(Clone, Debug, PartialEq, Eq, Hash, Serialize, Deserialize, Quine, Uniplate)]
#[path_prefix(conjure_cp::ast)]
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
    Matrix(Moo<GroundDomain>, Vec<Moo<GroundDomain>>),
    /// A tuple of N elements, each with its own domain
    Tuple(Vec<Moo<GroundDomain>>),
    Record(Vec<RecordEntryGround>),
}

impl GroundDomain {
    pub fn union(&self, other: &GroundDomain) -> Result<GroundDomain, DomainOpError> {
        match (self, other) {
            (GroundDomain::Empty(ty), dom) | (dom, GroundDomain::Empty(ty)) => {
                if *ty == dom.return_type() {
                    Ok(dom.clone())
                } else {
                    Err(DomainOpError::InputWrongType)
                }
            }
            (GroundDomain::Bool, GroundDomain::Bool) => Ok(GroundDomain::Bool),
            (GroundDomain::Bool, _) | (_, GroundDomain::Bool) => Err(DomainOpError::InputWrongType),
            (GroundDomain::Int(r1), GroundDomain::Int(r2)) => {
                let mut rngs = r1.clone();
                rngs.extend(r2.clone());
                Ok(GroundDomain::Int(Range::squeeze(&rngs)))
            }
            (GroundDomain::Int(_), _) | (_, GroundDomain::Int(_)) => {
                Err(DomainOpError::InputWrongType)
            }
            (GroundDomain::Set(_, in1), GroundDomain::Set(_, in2)) => Ok(GroundDomain::Set(
                SetAttr::default(),
                Moo::new(in1.union(in2)?),
            )),
            (GroundDomain::Set(_, _), _) | (_, GroundDomain::Set(_, _)) => {
                Err(DomainOpError::InputWrongType)
            }
            (GroundDomain::Matrix(in1, idx1), GroundDomain::Matrix(in2, idx2)) if idx1 == idx2 => {
                Ok(GroundDomain::Matrix(
                    Moo::new(in1.union(in2)?),
                    idx1.clone(),
                ))
            }
            (GroundDomain::Matrix(_, _), _) | (_, GroundDomain::Matrix(_, _)) => {
                Err(DomainOpError::InputWrongType)
            }
            (GroundDomain::Tuple(in1s), GroundDomain::Tuple(in2s)) if in1s.len() == in2s.len() => {
                let mut inners = Vec::new();
                for (in1, in2) in zip(in1s, in2s) {
                    inners.push(Moo::new(in1.union(in2)?));
                }
                Ok(GroundDomain::Tuple(inners))
            }
            (GroundDomain::Tuple(_), _) | (_, GroundDomain::Tuple(_)) => {
                Err(DomainOpError::InputWrongType)
            }
            // TODO: Eventually we may define semantics for joining record domains. This day is not today.
            (GroundDomain::Record(_), _) | (_, GroundDomain::Record(_)) => {
                Err(DomainOpError::InputWrongType)
            }
        }
    }

    pub fn intersect(&self, other: &GroundDomain) -> Result<GroundDomain, DomainOpError> {
        todo!()
    }

    pub fn values(&self) -> Result<Box<dyn Iterator<Item = Literal>>, DomainOpError> {
        match self {
            GroundDomain::Empty(_) => Ok(Box::new(vec![].into_iter())),
            GroundDomain::Bool => Ok(Box::new(
                vec![Literal::from(true), Literal::from(false)].into_iter(),
            )),
            GroundDomain::Int(rngs) => {
                let rng_iters = rngs
                    .iter()
                    .map(Range::iter)
                    .collect::<Option<Vec<_>>>()
                    .ok_or(DomainOpError::InputUnbounded)?;
                Ok(Box::new(
                    rng_iters.into_iter().flat_map(|ri| ri.map(Literal::from)),
                ))
            }
            _ => todo!("Enumerating nested domains is not yet supported"),
        }
    }

    pub fn contains(&self, value: &Literal) -> bool {
        todo!()
    }

    pub fn values_i32(&self) -> Result<Vec<i32>, DomainOpError> {
        todo!()
    }

    pub fn apply_i32(
        &self,
        op: fn(i32, i32) -> Option<i32>,
        other: &GroundDomain,
    ) -> Result<GroundDomain, DomainOpError> {
        todo!()
    }
}

impl Typeable for GroundDomain {
    fn return_type(&self) -> ReturnType {
        match self {
            GroundDomain::Empty(ty) => ty.clone(),
            GroundDomain::Bool => ReturnType::Bool,
            GroundDomain::Int(_) => ReturnType::Int,
            GroundDomain::Set(_attr, inner) => ReturnType::Set(Box::new(inner.return_type())),
            GroundDomain::Matrix(inner, _idx) => ReturnType::Matrix(Box::new(inner.return_type())),
            GroundDomain::Tuple(inners) => {
                let mut inner_types = Vec::new();
                for inner in inners {
                    inner_types.push(inner.return_type());
                }
                ReturnType::Tuple(inner_types)
            }
            GroundDomain::Record(entries) => {
                let mut entry_types = Vec::new();
                for entry in entries {
                    entry_types.push(entry.domain.return_type());
                }
                ReturnType::Record(entry_types)
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
