use super::{DomainPtr, GroundDomain, IntVal, Range};
use crate::ast::records::RecordValue;
use crate::ast::{
    Domain, DomainOpError, Expression, FuncAttr, MSetAttr, Moo, Reference, ReturnType, SetAttr,
    Typeable,
};
use crate::bug;
use funcmap::FuncMap;
use itertools::Itertools;
use polyquine::Quine;
use serde::{Deserialize, Serialize};
use std::fmt::{Display, Formatter};
use std::iter::zip;
use uniplate::Uniplate;

#[derive(Clone, Debug, PartialEq, Eq, Hash, Serialize, Deserialize, Quine, Uniplate)]
#[biplate(to=DomainPtr)]
#[biplate(to=Domain)]
#[biplate(to=GroundDomain)]
#[biplate(to=Expression)]
#[biplate(to=Reference)]
#[biplate(to=IntVal)]
#[path_prefix(conjure_cp::ast)]
pub enum UnresolvedDomain {
    Int(Vec<Range<IntVal>>),
    /// A set of elements drawn from the inner domain
    Set(SetAttr<IntVal>, DomainPtr),
    MSet(MSetAttr<IntVal>, DomainPtr),
    /// A n-dimensional matrix with a value domain and n-index domains
    Matrix(DomainPtr, Vec<DomainPtr>),
    /// A tuple of N elements, each with its own domain
    Tuple(Vec<DomainPtr>),
    /// A reference to a domain letting
    #[polyquine_skip]
    Reference(Reference),
    /// A record
    Record(Vec<RecordValue<Moo<Domain>>>),
    /// A function with attributes, domain, and range
    Function(FuncAttr<IntVal>, DomainPtr, DomainPtr),
}

impl UnresolvedDomain {
    pub fn resolve(&self) -> Result<GroundDomain, DomainOpError> {
        match self {
            UnresolvedDomain::Int(rngs) => rngs
                .iter()
                .map(Range::<IntVal>::resolve)
                .try_collect()
                .map(GroundDomain::Int),
            UnresolvedDomain::Set(attr, inner) => {
                Ok(GroundDomain::Set(attr.resolve_uint()?, inner.resolve()?))
            }
            UnresolvedDomain::MSet(attr, inner) => {
                Ok(GroundDomain::MSet(attr.resolve_uint()?, inner.resolve()?))
            }
            UnresolvedDomain::Matrix(inner, idx_doms) => {
                let inner_gd = inner.resolve()?;
                idx_doms
                    .iter()
                    .map(DomainPtr::resolve)
                    .try_collect()
                    .map(|idx| GroundDomain::Matrix(inner_gd, idx))
            }
            UnresolvedDomain::Tuple(inners) => inners
                .iter()
                .map(DomainPtr::resolve)
                .try_collect()
                .map(GroundDomain::Tuple),
            UnresolvedDomain::Record(entries) => entries
                .iter()
                .map(|f| {
                    f.value.resolve().map(|gd| RecordValue {
                        name: f.name.clone(),
                        value: gd,
                    })
                })
                .try_collect()
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
                attr.resolve_uint()?,
                dom.resolve()?,
                cdom.resolve()?,
            )),
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
            // TODO: Could we support unions of reference domains symbolically?
            (UnresolvedDomain::Reference(_), _) | (_, UnresolvedDomain::Reference(_)) => {
                Err(DomainOpError::NotGround)
            }
            // TODO: Could we define semantics for merging record domains?
            #[allow(unreachable_patterns)] // Technically redundant but logically makes sense
            (UnresolvedDomain::Record(_), _) | (_, UnresolvedDomain::Record(_)) => {
                Err(DomainOpError::WrongType)
            }
            #[allow(unreachable_patterns)]
            // Technically redundant but logically clearer to have both
            (UnresolvedDomain::Function(_, _, _), _) | (_, UnresolvedDomain::Function(_, _, _)) => {
                Err(DomainOpError::WrongType)
            }
        }
    }

    pub fn element_domain(&self) -> Option<DomainPtr> {
        match self {
            UnresolvedDomain::Set(_, inner_dom) => Some(inner_dom.clone()),
            UnresolvedDomain::Matrix(_, _) => {
                todo!("Unwrap one dimension of the domain")
            }
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
            UnresolvedDomain::Function(_, dom, cdom) => {
                ReturnType::Function(Box::new(dom.return_type()), Box::new(cdom.return_type()))
            }
        }
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
            UnresolvedDomain::Matrix(value_domain, index_domains) => {
                write!(
                    f,
                    "matrix indexed by [{}] of {value_domain}",
                    &index_domains.iter().join(", ")
                )
            }
            UnresolvedDomain::Tuple(domains) => {
                write!(f, "tuple of ({})", &domains.iter().join(", "))
            }
            UnresolvedDomain::Record(entries) => {
                write!(
                    f,
                    "record of ({})",
                    &entries
                        .iter()
                        .map(|entry| format!("{}: {}", entry.name, entry.value))
                        .join(", ")
                )
            }
            UnresolvedDomain::Function(attribute, domain, codomain) => {
                write!(f, "function {} {} --> {} ", attribute, domain, codomain)
            }
        }
    }
}
