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
/// Variants use the project-wide type/domain ordering; keep broad matches in the same order.
pub enum UnresolvedDomain {
    Int(Vec<Range<IntVal>>),
    /// A tuple of N elements, each with its own domain
    Tuple(Vec<DomainPtr>),
    /// A record
    Record(Vec<FieldUnresolved>),
    /// A variant domain with its domain options (reusing field entries)
    Variant(Vec<FieldUnresolved>),
    /// A n-dimensional matrix with a value domain and n-index domains
    Matrix(DomainPtr, Vec<DomainPtr>),
    Sequence(SequenceAttr<IntVal>, DomainPtr),
    /// A set of elements drawn from the inner domain
    Set(SetAttr<IntVal>, DomainPtr),
    MSet(MSetAttr<IntVal>, DomainPtr),
    /// A function with attributes, domain, and range
    Function(FuncAttr<IntVal>, DomainPtr, DomainPtr),
    /// A relation as a set of tuples
    Relation(RelAttr<IntVal>, Vec<DomainPtr>),
    Partition(PartitionAttr<IntVal>, DomainPtr),
    /// A reference to a domain letting
    #[polyquine_skip]
    Reference(Reference),
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
            UnresolvedDomain::Set(attr, inner) => {
                Ok(GroundDomain::Set(attr.resolve()?, inner.resolve()?))
            }
            UnresolvedDomain::MSet(attr, inner) => {
                Ok(GroundDomain::MSet(attr.resolve()?, inner.resolve()?))
            }
            UnresolvedDomain::Function(attr, dom, cdom) => Ok(GroundDomain::Function(
                attr.resolve()?,
                dom.resolve()?,
                cdom.resolve()?,
            )),
            UnresolvedDomain::Relation(attr, inners) => {
                let resolved_attr = attr.resolve()?;
                inners
                    .iter()
                    .map(DomainPtr::resolve)
                    .collect::<Result<_, _>>()
                    .map(|items| GroundDomain::Relation(resolved_attr, items))
            }
            UnresolvedDomain::Partition(attr, inner) => {
                Ok(GroundDomain::Partition(attr.resolve()?, inner.resolve()?))
            }
            UnresolvedDomain::Reference(re) => re
                .ptr
                .as_domain_letting()
                .unwrap_or_else(|| {
                    bug!("Reference domain should point to domain letting, but got {re}")
                })
                .resolve()
                .map(Moo::unwrap_or_clone),
        }
    }

    pub(super) fn union_unresolved(
        &self,
        other: &UnresolvedDomain,
    ) -> Result<UnresolvedDomain, DomainOpError> {
        // Keep implemented variants before unsupported variants so mixed-domain unions report the
        // established error. Each group uses declaration order.
        match (self, other) {
            (UnresolvedDomain::Int(lhs), UnresolvedDomain::Int(rhs)) => {
                let merged = lhs.iter().chain(rhs.iter()).cloned().collect_vec();
                Ok(UnresolvedDomain::Int(merged))
            }
            (UnresolvedDomain::Int(_), _) | (_, UnresolvedDomain::Int(_)) => {
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
            (UnresolvedDomain::Matrix(in1, idx1), UnresolvedDomain::Matrix(in2, idx2))
                if idx1 == idx2 =>
            {
                Ok(UnresolvedDomain::Matrix(in1.union(in2)?, idx1.clone()))
            }
            (UnresolvedDomain::Matrix(_, _), _) | (_, UnresolvedDomain::Matrix(_, _)) => {
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
            // TODO: Could we define semantics for merging record domains?
            #[allow(unreachable_patterns)]
            (UnresolvedDomain::Record(_), _) | (_, UnresolvedDomain::Record(_)) => {
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
            #[allow(unreachable_patterns)]
            (UnresolvedDomain::Function(_, _, _), _) | (_, UnresolvedDomain::Function(_, _, _)) => {
                Err(DomainOpError::WrongType)
            }
            #[allow(unreachable_patterns)]
            (UnresolvedDomain::Partition(_, _), _) | (_, UnresolvedDomain::Partition(_, _)) => {
                Err(DomainOpError::WrongType)
            }
            // TODO: Could we support unions of reference domains symbolically?
            #[allow(unreachable_patterns)]
            (UnresolvedDomain::Reference(_), _) | (_, UnresolvedDomain::Reference(_)) => {
                Err(DomainOpError::NotGround)
            }
        }
    }

    pub fn element_domain(&self) -> Option<DomainPtr> {
        match self {
            UnresolvedDomain::Matrix(inner, _) => Some(inner.clone()),
            UnresolvedDomain::Sequence(_, inner_dom) => Some(inner_dom.clone()),
            UnresolvedDomain::Set(_, inner_dom) => Some(inner_dom.clone()),
            _ => None,
        }
    }

    /// True if any set domain in this tree has a representation preference.
    pub fn has_representation_preference(&self) -> bool {
        match self {
            UnresolvedDomain::Int(_) => false,
            UnresolvedDomain::Tuple(inners) => {
                inners.iter().any(|d| d.has_representation_preference())
            }
            UnresolvedDomain::Record(entries) => entries
                .iter()
                .any(|f| f.value.has_representation_preference()),
            UnresolvedDomain::Variant(entries) => entries
                .iter()
                .any(|f| f.value.has_representation_preference()),
            UnresolvedDomain::Matrix(inner, idxs) => {
                inner.has_representation_preference()
                    || idxs.iter().any(|d| d.has_representation_preference())
            }
            UnresolvedDomain::Sequence(attr, inner) => {
                attr.representation.is_some() || inner.has_representation_preference()
            }
            UnresolvedDomain::Set(attr, inner) => {
                attr.representation.is_some() || inner.has_representation_preference()
            }
            UnresolvedDomain::MSet(_, inner) => inner.has_representation_preference(),
            UnresolvedDomain::Function(_, dom, cdom) => {
                dom.has_representation_preference() || cdom.has_representation_preference()
            }
            UnresolvedDomain::Relation(_, inners) => {
                inners.iter().any(|d| d.has_representation_preference())
            }
            UnresolvedDomain::Partition(_, inner) => inner.has_representation_preference(),
            UnresolvedDomain::Reference(re) => re
                .domain()
                .is_some_and(|d| d.has_representation_preference()),
        }
    }

    /// Format this domain in Essence type style, omitting size attributes and integer ranges.
    pub fn as_type_string(&self) -> String {
        match self {
            UnresolvedDomain::Int(_) => "int".to_string(),
            UnresolvedDomain::Tuple(inners) => {
                format!(
                    "tuple ({})",
                    inners.iter().map(|d| d.as_type_string()).join(", ")
                )
            }
            UnresolvedDomain::Record(entries) => {
                let inners = entries
                    .iter()
                    .map(|f| format!("{}: {}", f.name, f.value.as_type_string()))
                    .join(", ");
                format!("record {{{inners}}}")
            }
            UnresolvedDomain::Variant(entries) => {
                let inners = entries
                    .iter()
                    .map(|f| format!("{}: {}", f.name, f.value.as_type_string()))
                    .join(", ");
                format!("variant {{{inners}}}")
            }
            UnresolvedDomain::Matrix(inner, idxs) => {
                let idxs = idxs.iter().map(|d| d.as_type_string()).join(", ");
                format!("matrix indexed by [{idxs}] of {}", inner.as_type_string())
            }
            UnresolvedDomain::Sequence(_, inner) => {
                format!("sequence of {}", inner.as_type_string())
            }
            UnresolvedDomain::Set(attrs, inner) => {
                let mut out = String::from("set");
                if let Some(repr) = &attrs.representation {
                    out.push('{');
                    out.push_str(repr);
                    out.push('}');
                }
                out.push_str(" of ");
                out.push_str(&inner.as_type_string());
                out
            }
            UnresolvedDomain::MSet(_, inner) => format!("mset of {}", inner.as_type_string()),
            UnresolvedDomain::Function(_, dom, cdom) => {
                format!(
                    "function {} --> {}",
                    dom.as_type_string(),
                    cdom.as_type_string()
                )
            }
            UnresolvedDomain::Relation(_, inners) => {
                format!(
                    "relation of ({})",
                    inners.iter().map(|d| d.as_type_string()).join(" * ")
                )
            }
            UnresolvedDomain::Partition(_, inner) => {
                format!("partition from {}", inner.as_type_string())
            }
            UnresolvedDomain::Reference(re) => re.to_string(),
        }
    }
}

impl Typeable for UnresolvedDomain {
    fn return_type(&self) -> ReturnType {
        match self {
            UnresolvedDomain::Int(_) => ReturnType::Int,
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
            UnresolvedDomain::Matrix(inner, _idx) => {
                ReturnType::Matrix(Box::new(inner.return_type()))
            }
            UnresolvedDomain::Sequence(_attr, inner) => {
                ReturnType::Sequence(Box::new(inner.return_type()))
            }
            UnresolvedDomain::Set(_attr, inner) => ReturnType::Set(Box::new(inner.return_type())),
            UnresolvedDomain::MSet(_attr, inner) => ReturnType::MSet(Box::new(inner.return_type())),
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
            UnresolvedDomain::Partition(_, inner) => {
                ReturnType::Partition(Box::new(inner.return_type()))
            }
            UnresolvedDomain::Reference(re) => re.return_type(),
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
            UnresolvedDomain::Int(ranges) => {
                if ranges.iter().all(Range::is_lower_or_upper_bounded) {
                    let rngs: String = ranges.iter().map(|r| format!("{r}")).join(", ");
                    write!(f, "int({})", rngs)
                } else {
                    write!(f, "int")
                }
            }
            UnresolvedDomain::Tuple(domains) => {
                write!(f, "tuple ({})", domains.iter().join(","))
            }
            UnresolvedDomain::Record(entries) => {
                let inners = entries.iter().map(|t| format!("{}", t)).join(", ");
                write!(f, "record {{{inners}}}",)
            }
            UnresolvedDomain::Variant(entries) => {
                let inners = entries.iter().map(|t| format!("{}", t)).join(", ");
                write!(f, "variant {{{inners}}}",)
            }
            UnresolvedDomain::Matrix(value_domain, index_domains) => {
                write!(
                    f,
                    "matrix indexed by {} of {value_domain}",
                    pretty_vec(&index_domains.iter().collect_vec())
                )
            }
            UnresolvedDomain::Sequence(attrs, inner_dom) => {
                write!(f, "sequence {attrs} of {inner_dom}")
            }
            UnresolvedDomain::Set(attrs, inner_dom) => {
                write!(f, "set")?;
                if let Some(repr) = &attrs.representation {
                    write!(f, "{{{repr}}}")?;
                }
                let attrs = attrs.to_string();
                if attrs.is_empty() {
                    write!(f, " of {inner_dom}")
                } else {
                    write!(f, " {attrs} of {inner_dom}")
                }
            }
            UnresolvedDomain::MSet(attrs, inner_dom) => write!(f, "mset {attrs} of {inner_dom}"),
            UnresolvedDomain::Function(attribute, domain, codomain) => {
                write!(f, "function {} {} --> {} ", attribute, domain, codomain)
            }
            UnresolvedDomain::Relation(attrs, domains) => {
                write!(f, "relation {} of ({})", attrs, domains.iter().join(" * "))
            }
            UnresolvedDomain::Partition(attrs, inner_dom) => {
                write!(f, "partition {attrs} from {inner_dom}")
            }
            UnresolvedDomain::Reference(re) => write!(f, "{re}"),
        }
    }
}
