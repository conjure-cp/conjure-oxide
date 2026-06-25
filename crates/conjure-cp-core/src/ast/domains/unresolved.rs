use std::fmt::{Display, Formatter};
use std::iter::zip;

use crate::ast::domains::attrs::MSetAttr;
use crate::ast::domains::attrs::PartitionAttr;
use crate::ast::domains::attrs::SetAttr;
use crate::ast::domains::ground::FieldGround;
use crate::ast::records::Field;
use crate::ast::{
    DomainOpError, Expression, FuncAttr, Moo, Reference, RelAttr, ReturnType, SequenceAttr,
    Typeable,
    domains::{DomainPtr, GroundDomain, int_val::IntVal, range::Range},
    pretty::pretty_vec,
};
use crate::bug;

use funcmap::{FuncMap, TryFuncMap};
use itertools::Itertools;
use polyquine::Quine;
use serde::{Deserialize, Serialize};
use uniplate::Uniplate;

pub(super) type FieldUnresolved = Field<DomainPtr>;

impl From<FieldGround> for FieldUnresolved {
    fn from(v: FieldGround) -> Self {
        v.func_map(DomainPtr::from)
    }
}

impl TryFrom<FieldUnresolved> for FieldGround {
    type Error = DomainOpError;
    fn try_from(v: FieldUnresolved) -> Result<Self, Self::Error> {
        v.try_func_map(DomainPtr::try_into)
    }
}

#[derive(Clone, Debug, PartialEq, Eq, Hash, Serialize, Deserialize, Quine, Uniplate)]
#[path_prefix(conjure_cp::ast)]
#[biplate(to=Expression)]
#[biplate(to=Reference)]
#[biplate(to=IntVal)]
#[biplate(to=DomainPtr)]
pub enum UnresolvedDomain {
    Int(Vec<Range<IntVal>>),
    /// A set of elements drawn from the inner domain
    Set(SetAttr<IntVal>, DomainPtr),
    MSet(MSetAttr<IntVal>, DomainPtr),
    /// A n-dimensional matrix with a value domain and n-index domains
    Matrix(DomainPtr, Vec<DomainPtr>),
    /// A tuple of N elements, each with its own domain
    Tuple(Vec<DomainPtr>),
    Sequence(SequenceAttr<IntVal>, DomainPtr),
    /// A reference to a domain letting
    #[polyquine_skip]
    Reference(Reference),
    /// A record
    Record(Vec<FieldUnresolved>),
    /// A function with attributes, domain, and range
    Function(FuncAttr<IntVal>, DomainPtr, DomainPtr),
    /// A variant domain with its domain options (reusing field entries)
    Variant(Vec<FieldUnresolved>),
    /// A relation as a set of tuples
    Relation(RelAttr<IntVal>, Vec<DomainPtr>),
    Partition(PartitionAttr<IntVal>, DomainPtr),
}

impl UnresolvedDomain {
    pub fn resolve(&self) -> Result<GroundDomain, DomainOpError> {
        match self {
            UnresolvedDomain::Int(rngs) => rngs
                .iter()
                .map(Range::<IntVal>::resolve)
                .collect::<Result<Vec<_>, _>>()
                .map(|ranges| {
                    let ranges = ranges
                        .into_iter()
                        .filter(
                            |range| !matches!(range, Range::Bounded(lower, upper) if lower > upper),
                        )
                        .collect::<Vec<_>>();
                    GroundDomain::Int(Range::squeeze(&ranges))
                }),
            UnresolvedDomain::Set(attr, inner) => {
                Ok(GroundDomain::Set(attr.resolve()?, inner.resolve()?))
            }
            UnresolvedDomain::MSet(attr, inner) => {
                Ok(GroundDomain::MSet(attr.resolve()?, inner.resolve()?))
            }
            UnresolvedDomain::Partition(attr, inner) => {
                Ok(GroundDomain::Partition(attr.resolve()?, inner.resolve()?))
            }
            UnresolvedDomain::Matrix(inner, idx_doms) => {
                let inner_gd = inner.resolve()?;
                idx_doms
                    .iter()
                    .map(DomainPtr::resolve)
                    .collect::<Result<_, _>>()
                    .map(|idx| GroundDomain::Matrix(inner_gd, idx))
            }
            UnresolvedDomain::Sequence(attr, inner) => {
                Ok(GroundDomain::Sequence(attr.resolve()?, inner.resolve()?))
            }
            UnresolvedDomain::Tuple(inners) => inners
                .iter()
                .map(DomainPtr::resolve)
                .collect::<Result<_, _>>()
                .map(GroundDomain::Tuple),
            UnresolvedDomain::Record(entries) => entries
                .iter()
                .map(|f| {
                    f.value.resolve().map(|gd| FieldGround {
                        name: f.name.clone(),
                        value: gd,
                    })
                })
                .collect::<Result<_, _>>()
                .map(GroundDomain::Record),
            UnresolvedDomain::Reference(re) => re
                .ptr
                .as_domain_letting()
                .unwrap_or_else(|| {
                    bug!("Reference domain should point to domain letting, but got {re}")
                })
                .resolve()
                .map(Moo::unwrap_or_clone),
            UnresolvedDomain::Function(attr, dom, cdom) => Ok(GroundDomain::Function(
                attr.resolve()?,
                dom.resolve()?,
                cdom.resolve()?,
            )),
            UnresolvedDomain::Variant(entries) => entries
                .iter()
                .map(|f| {
                    f.value.resolve().map(|gd| FieldGround {
                        name: f.name.clone(),
                        value: gd,
                    })
                })
                .collect::<Result<_, _>>()
                .map(GroundDomain::Variant),
            UnresolvedDomain::Relation(attr, inners) => {
                let resolved_attr = attr.resolve()?;
                inners
                    .iter()
                    .map(DomainPtr::resolve)
                    .collect::<Result<_, _>>()
                    .map(|items| GroundDomain::Relation(resolved_attr, items))
            }
        }
    }

    pub(super) fn union_unresolved(
        &self,
        other: &UnresolvedDomain,
    ) -> Result<UnresolvedDomain, DomainOpError> {
        match (self, other) {
            (UnresolvedDomain::Int(lhs), UnresolvedDomain::Int(rhs)) => {
                let merged = lhs.iter().chain(rhs.iter()).cloned().collect_vec();
                Ok(UnresolvedDomain::Int(merged))
            }
            (UnresolvedDomain::Int(_), _) | (_, UnresolvedDomain::Int(_)) => {
                Err(DomainOpError::WrongType)
            }
            (UnresolvedDomain::Set(_, in1), UnresolvedDomain::Set(_, in2)) => {
                Ok(UnresolvedDomain::Set(SetAttr::default(), in1.union(in2)?))
            }
            (UnresolvedDomain::Set(_, _), _) | (_, UnresolvedDomain::Set(_, _)) => {
                Err(DomainOpError::WrongType)
            }
            (UnresolvedDomain::MSet(_, in1), UnresolvedDomain::MSet(_, in2)) => {
                Ok(UnresolvedDomain::MSet(MSetAttr::default(), in1.union(in2)?))
            }
            (UnresolvedDomain::MSet(_, _), _) | (_, UnresolvedDomain::MSet(_, _)) => {
                Err(DomainOpError::WrongType)
            }
            (UnresolvedDomain::Matrix(in1, idx1), UnresolvedDomain::Matrix(in2, idx2))
                if idx1 == idx2 =>
            {
                Ok(UnresolvedDomain::Matrix(in1.union(in2)?, idx1.clone()))
            }
            (UnresolvedDomain::Matrix(_, _), _) | (_, UnresolvedDomain::Matrix(_, _)) => {
                Err(DomainOpError::WrongType)
            }
            (UnresolvedDomain::Tuple(lhs), UnresolvedDomain::Tuple(rhs))
                if lhs.len() == rhs.len() =>
            {
                let mut merged = Vec::new();
                for (l, r) in zip(lhs, rhs) {
                    merged.push(l.union(r)?)
                }
                Ok(UnresolvedDomain::Tuple(merged))
            }
            (UnresolvedDomain::Tuple(_), _) | (_, UnresolvedDomain::Tuple(_)) => {
                Err(DomainOpError::WrongType)
            }
            (UnresolvedDomain::Relation(_, in1s), UnresolvedDomain::Relation(_, in2s)) => {
                let mut inners = Vec::new();
                for (in1, in2) in in1s.iter().zip(in2s.iter()) {
                    inners.push(in1.union(in2)?)
                }
                Ok(UnresolvedDomain::Relation(RelAttr::default(), inners))
            }
            (UnresolvedDomain::Relation(_, _), _) | (_, UnresolvedDomain::Relation(_, _)) => {
                Err(DomainOpError::WrongType)
            }
            // TODO: Could we support unions of reference domains symbolically?
            (UnresolvedDomain::Reference(_), _) | (_, UnresolvedDomain::Reference(_)) => {
                Err(DomainOpError::NotGround)
            }
            // TODO: Could we define semantics for merging record domains?
            #[allow(unreachable_patterns)]
            (UnresolvedDomain::Record(_), _) | (_, UnresolvedDomain::Record(_)) => {
                Err(DomainOpError::WrongType)
            }
            #[allow(unreachable_patterns)]
            (UnresolvedDomain::Function(_, _, _), _) | (_, UnresolvedDomain::Function(_, _, _)) => {
                Err(DomainOpError::WrongType)
            }
            #[allow(unreachable_patterns)]
            (UnresolvedDomain::Partition(_, _), _) | (_, UnresolvedDomain::Partition(_, _)) => {
                Err(DomainOpError::WrongType)
            }
            #[allow(unreachable_patterns)]
            (UnresolvedDomain::Variant(_), _) | (_, UnresolvedDomain::Variant(_)) => {
                Err(DomainOpError::WrongType)
            }
            #[allow(unreachable_patterns)]
            (UnresolvedDomain::Sequence(_, _), _) | (_, UnresolvedDomain::Sequence(_, _)) => {
                Err(DomainOpError::WrongType)
            }
        }
    }

    pub fn element_domain(&self) -> Option<DomainPtr> {
        match self {
            UnresolvedDomain::Set(_, inner_dom) => Some(inner_dom.clone()),
            UnresolvedDomain::Sequence(_, inner_dom) => Some(inner_dom.clone()),
            UnresolvedDomain::Matrix(inner, _) => Some(inner.clone()),
            _ => None,
        }
    }
}

impl Typeable for UnresolvedDomain {
    fn return_type(&self) -> ReturnType {
        match self {
            UnresolvedDomain::Reference(re) => re.return_type(),
            UnresolvedDomain::Int(_) => ReturnType::Int,
            UnresolvedDomain::Set(_attr, inner) => ReturnType::Set(Box::new(inner.return_type())),
            UnresolvedDomain::MSet(_attr, inner) => ReturnType::MSet(Box::new(inner.return_type())),
            UnresolvedDomain::Partition(_, inner) => {
                ReturnType::Partition(Box::new(inner.return_type()))
            }
            UnresolvedDomain::Sequence(_attr, inner) => {
                ReturnType::Sequence(Box::new(inner.return_type()))
            }
            UnresolvedDomain::Matrix(inner, _idx) => {
                ReturnType::Matrix(Box::new(inner.return_type()))
            }
            UnresolvedDomain::Tuple(inners) => {
                let mut inner_types = Vec::new();
                for inner in inners {
                    inner_types.push(inner.return_type());
                }
                ReturnType::Tuple(inner_types)
            }
            UnresolvedDomain::Record(entries) => {
                let mut entry_types = Vec::new();
                for entry in entries {
                    entry_types.push(entry.clone().func_map(|x| x.return_type()));
                }
                ReturnType::Record(entry_types)
            }
            UnresolvedDomain::Variant(entries) => {
                let mut entry_types = Vec::new();
                for entry in entries {
                    entry_types.push(entry.clone().func_map(|x| x.return_type()));
                }
                ReturnType::Variant(entry_types)
            }
            UnresolvedDomain::Function(_, dom, cdom) => {
                ReturnType::Function(Box::new(dom.return_type()), Box::new(cdom.return_type()))
            }
            UnresolvedDomain::Relation(_, inners) => {
                let mut inner_types = Vec::new();
                for inner in inners {
                    inner_types.push(inner.return_type());
                }
                ReturnType::Relation(inner_types)
            }
        }
    }
}

impl Display for FieldUnresolved {
    fn fmt(&self, f: &mut Formatter<'_>) -> std::fmt::Result {
        write!(f, "{}: {}", self.name, self.value)
    }
}

impl Display for UnresolvedDomain {
    fn fmt(&self, f: &mut Formatter<'_>) -> std::fmt::Result {
        match &self {
            UnresolvedDomain::Reference(re) => write!(f, "{re}"),
            UnresolvedDomain::Int(ranges) => {
                if ranges.iter().all(Range::is_lower_or_upper_bounded) {
                    let rngs: String = ranges.iter().map(|r| format!("{r}")).join(", ");
                    write!(f, "int({})", rngs)
                } else {
                    write!(f, "int")
                }
            }
            UnresolvedDomain::Set(attrs, inner_dom) => write!(f, "set {attrs} of {inner_dom}"),
            UnresolvedDomain::MSet(attrs, inner_dom) => write!(f, "mset {attrs} of {inner_dom}"),
            UnresolvedDomain::Partition(attrs, inner_dom) => {
                write!(f, "partition {attrs} from {inner_dom}")
            }
            UnresolvedDomain::Sequence(attrs, inner_dom) => {
                write!(f, "sequence {attrs} of {inner_dom}")
            }
            UnresolvedDomain::Matrix(value_domain, index_domains) => {
                write!(
                    f,
                    "matrix indexed by {} of {value_domain}",
                    pretty_vec(&index_domains.iter().collect_vec())
                )
            }
            UnresolvedDomain::Tuple(domains) => {
                write!(f, "tuple ({})", &domains.iter().join(","))
            }
            UnresolvedDomain::Record(entries) => {
                let inners = entries.iter().map(|t| format!("{}", t)).join(", ");
                write!(f, "record {{{inners}}}",)
            }
            UnresolvedDomain::Variant(entries) => {
                let inners = entries.iter().map(|t| format!("{}", t)).join(", ");
                write!(f, "variant {{{inners}}}",)
            }
            UnresolvedDomain::Function(attribute, domain, codomain) => {
                write!(f, "function {} {} --> {} ", attribute, domain, codomain)
            }
            UnresolvedDomain::Relation(attrs, domains) => {
                write!(f, "relation {} of ({})", attrs, domains.iter().join(" * "))
            }
        }
    }
}
