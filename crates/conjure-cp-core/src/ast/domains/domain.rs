use crate::ast::domains::ground::GroundDomain;
use crate::ast::domains::range::Range;
use crate::ast::domains::set_attr::SetAttr;
use crate::ast::domains::unresolved::{IntVal, UnresolvedDomain};
use crate::ast::{DomainOpError, MaybeTypeable, Moo, ReturnType, Typeable};
use itertools::Itertools;
use polyquine::Quine;
use serde::{Deserialize, Serialize};
use std::fmt::{Display, Formatter};

/// The integer type used in all domain code (int ranges, set sizes, etc)
pub type Int = i32;

pub type DomainPtr = Moo<Domain>;

impl DomainPtr {
    pub fn resolve(&self) -> Option<Moo<GroundDomain>> {
        self.as_ref().resolve()
    }
}

impl Into<DomainPtr> for Moo<GroundDomain> {
    fn into(self) -> DomainPtr {
        Moo::new(Domain::Ground(self))
    }
}

impl Into<DomainPtr> for Moo<UnresolvedDomain> {
    fn into(self) -> DomainPtr {
        Moo::new(Domain::Unresolved(self))
    }
}

#[derive(Clone, Debug, PartialEq, Eq, Hash, Serialize, Deserialize, Quine)]
pub enum Domain {
    /// A fully resolved domain
    Ground(Moo<GroundDomain>),
    /// A domain which may contain references
    Unresolved(Moo<UnresolvedDomain>),
}

/// Types that have a [`Domain`].
pub trait HasDomain {
    /// Gets the [`Domain`] of `self`.
    fn domain_of(&self) -> DomainPtr;
}

impl<T: HasDomain> MaybeTypeable for T {
    fn maybe_return_type(&self) -> Option<ReturnType> {
        self.domain_of().maybe_return_type()
    }
}

impl Domain {
    pub fn new_bool() -> DomainPtr {
        // TODO(perf): Since this is completely static, and we're using references, we may save
        // some minor memory allocations by initialising one static Moo::(...Bool)
        // and passing that around instead of creating new ones every time
        Moo::new(Domain::Ground(Moo::new(GroundDomain::Bool)))
    }

    pub fn new_empty(ty: ReturnType) -> DomainPtr {
        Moo::new(Domain::Ground(Moo::new(GroundDomain::Empty(ty))))
    }

    pub fn new_int<T>(ranges: Vec<T>) -> DomainPtr
    where
        T: Into<Range<IntVal>> + TryInto<Range<Int>> + Clone,
    {
        if let Ok(int_rngs) = ranges.iter().cloned().map(TryInto::try_into).try_collect() {
            return Moo::new(Domain::Ground(Moo::new(GroundDomain::Int(int_rngs))));
        }
        let unresolved_rngs: Vec<Range<IntVal>> = ranges.into_iter().map(Into::into).collect();
        Moo::new(Domain::Unresolved(Moo::new(UnresolvedDomain::Int(
            unresolved_rngs,
        ))))
    }

    pub fn new_set<T>(attr: T, inner_dom: DomainPtr) -> DomainPtr
    where
        T: Into<SetAttr<IntVal>> + TryInto<SetAttr<Int>> + Clone,
    {
        if let Domain::Ground(gd) = inner_dom.as_ref()
            && let Ok(int_attr) = attr.clone().try_into()
        {
            return Moo::new(Domain::Ground(Moo::new(GroundDomain::Set(
                int_attr,
                gd.clone(),
            ))));
        }
        Moo::new(Domain::Unresolved(Moo::new(UnresolvedDomain::Set(
            attr.into(),
            inner_dom,
        ))))
    }

    pub fn new_matrix(inner_dom: DomainPtr, idx_doms: Vec<DomainPtr>) -> DomainPtr {
        if let Domain::Ground(gd) = inner_dom.as_ref()
            && let Some(idx_gds) = as_grounds(&idx_doms)
        {
            return Moo::new(Domain::Ground(Moo::new(GroundDomain::Matrix(
                gd.clone(),
                idx_gds,
            ))));
        }
        Moo::new(Domain::Unresolved(Moo::new(UnresolvedDomain::Matrix(
            inner_dom, idx_doms,
        ))))
    }

    pub fn new_tuple(inner_doms: Vec<DomainPtr>) -> DomainPtr {
        if let Some(inner_gds) = as_grounds(&inner_doms) {
            return Moo::new(Domain::Ground(Moo::new(GroundDomain::Tuple(inner_gds))));
        }
        Moo::new(Domain::Unresolved(Moo::new(UnresolvedDomain::Tuple(
            inner_doms,
        ))))
    }

    pub fn resolve(&self) -> Option<Moo<GroundDomain>> {
        match self {
            Domain::Ground(gd) => Some(gd.clone()),
            Domain::Unresolved(ud) => ud.resolve().map(Moo::new),
        }
    }

    pub fn as_ground(&self) -> Option<&GroundDomain> {
        match self {
            Domain::Ground(gd) => Some(gd.as_ref()),
            _ => None,
        }
    }

    pub fn as_ground_mut(&mut self) -> Option<&mut GroundDomain> {
        match self {
            Domain::Ground(gd) => Some(Moo::<GroundDomain>::make_mut(gd)),
            _ => None,
        }
    }

    pub fn as_unresolved(&self) -> Option<&UnresolvedDomain> {
        match self {
            Domain::Unresolved(ud) => Some(ud.as_ref()),
            _ => None,
        }
    }

    pub fn as_unresolved_mut(&mut self) -> Option<&mut UnresolvedDomain> {
        match self {
            Domain::Unresolved(ud) => Some(Moo::<UnresolvedDomain>::make_mut(ud)),
            _ => None,
        }
    }

    pub fn as_dom_int(&self) -> Option<&Vec<Range<IntVal>>> {
        if let Some(GroundDomain::Int(rngs)) = self.as_ground() {
            todo!()
        }
        if let Some(UnresolvedDomain::Int(rngs)) = self.as_unresolved() {
            return Some(rngs);
        }
        None
    }

    pub fn as_dom_matrix(&self) -> Option<(DomainPtr, Vec<DomainPtr>)> {
        if let Some(GroundDomain::Matrix(inner_dom_gd, idx_doms_gds)) = self.as_ground() {
            let idx_doms: Vec<DomainPtr> = idx_doms_gds.iter().cloned().map(|d| d.into()).collect();
            let inner_dom: DomainPtr = inner_dom_gd.clone().into();
            return Some((inner_dom, idx_doms));
        }
        if let Some(UnresolvedDomain::Matrix(inner_dom, idx_doms)) = self.as_unresolved() {
            return Some((inner_dom.clone(), idx_doms.clone()));
        }
        None
    }

    pub fn union(&self, other: &Domain) -> Result<Domain, DomainOpError> {
        match (self, other) {
            (Domain::Ground(a), Domain::Ground(b)) => Ok(Domain::Ground(Moo::new(a.union(b)?))),
            (Domain::Unresolved(a), Domain::Unresolved(b)) => {
                Ok(Domain::Unresolved(Moo::new(a.union_unresolved(b)?)))
            }
            (Domain::Unresolved(u), Domain::Ground(g))
            | (Domain::Ground(g), Domain::Unresolved(u)) => {
                todo!("Union of unresolved domain {u} and ground domain {g} is not yet implemented")
            }
        }
    }

    pub fn intersect(&self, other: &Domain) -> Result<Domain, DomainOpError> {
        todo!()
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

fn as_grounds(doms: &Vec<DomainPtr>) -> Option<Vec<Moo<GroundDomain>>> {
    doms.iter()
        .map(|idx| match idx.as_ref() {
            Domain::Ground(idx_gd) => Some(idx_gd.clone()),
            _ => None,
        })
        .collect()
}
