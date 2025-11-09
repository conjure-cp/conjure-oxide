use crate::ast::domains::ground::GroundDomain;
use crate::ast::domains::range::Range;
use crate::ast::domains::set_attr::SetAttr;
use crate::ast::domains::unresolved::{IntVal, UnresolvedDomain};
use crate::ast::{DomainOpError, MaybeTypeable, Moo, ReturnType, Typeable};
use itertools::Itertools;
use polyquine::Quine;
use serde::{Deserialize, Serialize};
use std::fmt::{Display, Formatter};
use std::ops::Deref;

/// The integer type used in all domain code (int ranges, set sizes, etc)
pub(crate) type Int = i32;

#[derive(Clone, Debug, PartialEq, Eq, Hash, Serialize, Deserialize, Quine)]
pub enum Domain {
    /// A fully resolved domain
    Ground(GroundDomain),
    /// A domain which may contain references
    Unresolved(UnresolvedDomain),
}

/// Types that have a [`Domain`].
pub trait HasDomain {
    /// Gets the [`Domain`] of `self`.
    fn domain_of(&self) -> Domain;
}

impl<T: HasDomain> MaybeTypeable for T {
    fn maybe_return_type(&self) -> Option<ReturnType> {
        self.domain_of().maybe_return_type()
    }
}

impl Domain {
    pub fn new_bool() -> Domain {
        Domain::Ground(GroundDomain::Bool)
    }

    pub fn new_empty(ty: ReturnType) -> Domain {
        Domain::Ground(GroundDomain::Empty(ty))
    }

    pub fn new_int<T>(ranges: Vec<T>) -> Domain
    where
        T: Into<Range<IntVal>> + TryInto<Range<Int>> + Clone,
    {
        if let Ok(int_rngs) = ranges.iter().cloned().map(TryInto::try_into).try_collect() {
            return Domain::Ground(GroundDomain::Int(int_rngs));
        }
        let unresolved_rngs: Vec<Range<IntVal>> = ranges.into_iter().map(Into::into).collect();
        Domain::Unresolved(UnresolvedDomain::Int(unresolved_rngs))
    }

    pub fn new_set<T>(attr: T, inner_dom: Moo<Domain>) -> Domain
    where
        T: Into<SetAttr<IntVal>> + TryInto<SetAttr<Int>> + Clone,
    {
        if let Domain::Ground(gd) = inner_dom.as_ref()
            && let Ok(int_attr) = attr.clone().try_into()
        {
            return Domain::Ground(GroundDomain::Set(int_attr, Moo::new(gd.clone())));
        }
        Domain::Unresolved(UnresolvedDomain::Set(attr.into(), inner_dom))
    }

    pub fn new_matrix(inner_dom: Moo<Domain>, idx_doms: Vec<Domain>) -> Domain {
        if let Domain::Ground(gd) = inner_dom.as_ref()
            && let Some(idx_gds) = as_grounds(&idx_doms)
        {
            return Domain::Ground(GroundDomain::Matrix(Moo::new(gd.clone()), idx_gds));
        }
        Domain::Unresolved(UnresolvedDomain::Matrix(inner_dom, idx_doms))
    }

    pub fn new_tuple(inner_doms: Vec<Domain>) -> Domain {
        if let Some(inner_gds) = as_grounds(&inner_doms) {
            return Domain::Ground(GroundDomain::Tuple(inner_gds));
        }
        Domain::Unresolved(UnresolvedDomain::Tuple(inner_doms))
    }

    pub fn resolve(&self) -> Option<GroundDomain> {
        match self {
            Domain::Ground(gd) => Some(gd.clone()),
            Domain::Unresolved(ud) => ud.resolve(),
        }
    }

    pub fn union(&self, other: &Domain) -> Result<Domain, DomainOpError> {
        match (self, other) {
            (Domain::Ground(a), Domain::Ground(b)) => Ok(Domain::Ground(a.union(b)?)),
            (Domain::Unresolved(a), Domain::Unresolved(b)) => {
                Ok(Domain::Unresolved(a.union_unresolved(b)?))
            }
            (Domain::Unresolved(u), Domain::Ground(g))
            | (Domain::Ground(g), Domain::Unresolved(u)) => {
                todo!("Union of unresolved domain {u} and ground domain {g} is not yet implemented")
            }
        }
    }
}

impl MaybeTypeable for Domain {
    fn maybe_return_type(&self) -> Option<ReturnType> {
        match self {
            Domain::Ground(dom) => Some(dom.return_type()),
            Domain::Unresolved(dom) => dom.maybe_return_type(),
        }
    }
}

impl Display for Domain {
    fn fmt(&self, f: &mut Formatter<'_>) -> std::fmt::Result {
        match &self {
            Domain::Ground(gd) => gd.fmt(f),
            Domain::Unresolved(ud) => ud.fmt(f),
        }
    }
}

fn as_grounds(doms: &Vec<Domain>) -> Option<Vec<GroundDomain>> {
    doms.iter()
        .map(|idx| match idx {
            Domain::Ground(idx_gd) => Some(idx_gd.clone()),
            _ => None,
        })
        .collect()
}
